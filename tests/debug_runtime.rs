#![cfg(feature = "debug-runtime")]
use minipy_native::{
    buffer::{Quote, QuoteBuffer},
    error::TaskError,
    interpreter::{InterpreterUnit, debug::Engine},
    ir::Type,
    provider::{Delays, Provider, TestProvider},
    runtime::{Cancellation, ScriptLimits},
};
use serde_json::Value;
use std::{rc::Rc, sync::Arc};
const SCAN: &str = "quotes=tools.load_quotes(batch)\ntotal=0\nfor q in quotes:\n    total=total+q.unit_price_cents*q.quantity\nreceipt=tools.record_total(batch,total)\nprint(receipt)";
struct EchoProvider(TestProvider);
impl Provider for EchoProvider {
    async fn load_quotes(&self, batch: i64) -> Result<QuoteBuffer, TaskError> {
        self.0.load_quotes(batch).await
    }
    async fn record_total(&self, _: i64, total: i64) -> Result<i64, TaskError> {
        Ok(total)
    }
}
fn create(source: &str, limits: ScriptLimits) -> (Engine, usize) {
    let mut e = Engine::default();
    let u = e.prepare(source, &[("batch", Type::I64)]).unwrap()["unit"]
        .as_u64()
        .unwrap() as usize;
    let t = e.create(u, &[2], limits).unwrap()["task"].as_u64().unwrap() as usize;
    (e, t)
}
fn finish(e: &mut Engine, t: usize, trace: bool) -> (Value, Vec<Value>) {
    let mut events = Vec::new();
    for _ in 0..1000 {
        let r = e.advance(t, if trace { 1 } else { 2048 }, trace).unwrap();
        events.extend(r["events"].as_array().unwrap().clone());
        let status = r["snapshot"]["status"].as_str().unwrap();
        if ["done", "error", "cancelled"].contains(&status) {
            return (r["snapshot"].clone(), events);
        }
        if status.ends_with("_wait") {
            let ev = r["events"].as_array().unwrap().last().unwrap();
            let req = ev["detail"]["tool"]["request"].as_u64().unwrap();
            let rows = if status == "load_wait" {
                Some(vec![
                    Quote {
                        unit_price_cents: 7,
                        quantity: 3
                    };
                    5
                ])
            } else {
                None
            };
            let receipt = ev["detail"]["tool"]["total"]
                .as_str()
                .map(|x| x.parse().unwrap());
            let r = e.complete(t, req, rows, receipt, None).unwrap();
            events.extend(r["events"].as_array().unwrap().clone());
        }
    }
    panic!("execution failed to terminate");
}
#[tokio::test]
async fn instruction_and_fast_dispatch_match_native_interpreter() {
    for source in [
        "x=3\nx=(x+1)*(x-1)+x*2\nprint(x)",
        "print(False and (9223372036854775807+1>0))\nprint(True or (9223372036854775807+1>0))",
        SCAN,
        "print(7)\nprint(9223372036854775807+1)",
    ] {
        let mut l = ScriptLimits::default();
        l.limits.chunk = 2;
        let native = minipy_native::interpreter::execute_script(
            Rc::new(InterpreterUnit::prepare_script(source, &[("batch", Type::I64)]).unwrap()),
            Arc::new(EchoProvider(TestProvider::new(
                vec![
                    Quote {
                        unit_price_cents: 7,
                        quantity: 3
                    };
                    5
                ],
                Delays::default(),
            ))),
            &[2],
            l,
            Cancellation::default(),
            None,
        )
        .await;
        let (mut e, t) = create(source, l);
        let (s, events) = finish(&mut e, t, true);
        assert_eq!(s["stdout"], native.stdout);
        assert_eq!(
            s["error"].as_str(),
            native
                .status
                .as_ref()
                .err()
                .map(|e| e.to_string())
                .as_deref()
        );
        assert_eq!(
            s["status"],
            if native.status.is_ok() {
                "done"
            } else {
                "error"
            }
        );
        assert!(
            events
                .windows(2)
                .all(|w| w[0]["id"].as_u64() < w[1]["id"].as_u64())
        );
        let (mut e, t) = create(source, l);
        let (fast, _) = finish(&mut e, t, false);
        assert_eq!(s["stdout"], fast["stdout"]);
        assert_eq!(s["status"], fast["status"]);
    }
}
#[test]
fn step_retains_scratch_and_reports_real_memory() {
    let (mut e, t) = create(SCAN, ScriptLimits::default());
    let r = e.advance(t, 1, true).unwrap();
    assert_eq!(r["snapshot"]["pc"], 1);
    let req = r["events"][0]["detail"]["tool"]["request"]
        .as_u64()
        .unwrap();
    e.complete(
        t,
        req,
        Some(vec![
            Quote {
                unit_price_cents: 7,
                quantity: 3
            };
            5
        ]),
        None,
        None,
    )
    .unwrap();
    let b = e.snapshot(t).unwrap()["buffers"][0].clone();
    assert_eq!(b["bytes"], 80);
    let (s, events) = finish(&mut e, t, true);
    assert_eq!(s["stdout"], "105\n");
    assert_eq!(s["buffers"], serde_json::json!([]));
    let reads = events
        .iter()
        .filter_map(|e| e["detail"]["execution"]["read"].as_object())
        .collect::<Vec<_>>();
    assert_eq!(reads.len(), 5);
    for (i, r) in reads.iter().enumerate() {
        assert_eq!(
            r["offset"].as_u64().unwrap(),
            b["offset"].as_u64().unwrap() + i as u64 * 16
        );
    }
}
#[test]
fn cancellation_isolated_and_stale_token_rejected() {
    let (mut e, a) = create(SCAN, ScriptLimits::default());
    let b = e.create(0, &[2], ScriptLimits::default()).unwrap()["task"]
        .as_u64()
        .unwrap() as usize;
    let r = e.advance(a, 1, true).unwrap();
    let req = r["events"][0]["detail"]["tool"]["request"]
        .as_u64()
        .unwrap();
    assert!(e.complete(a, req + 1, Some(vec![]), None, None).is_err());
    e.cancel(a).unwrap();
    assert_eq!(
        e.complete(a, req, Some(vec![]), None, None).unwrap()["ignored"],
        true
    );
    assert_eq!(finish(&mut e, b, true).0["stdout"], "105\n");
    assert_eq!(e.snapshot(a).unwrap()["status"], "cancelled");
}
#[test]
fn limits_permissions_and_provider_errors_are_task_errors() {
    for mode in 0..5 {
        let mut l = ScriptLimits::default();
        match mode {
            0 => l.limits.allow_load = false,
            1 => l.tool_calls = 0,
            2 => l.limits.rows = 2,
            3 => l.limits.buffer_bytes = 16,
            _ => l.stdout_bytes = 1,
        }
        let (mut e, t) = create(SCAN, l);
        let (s, _) = finish(&mut e, t, true);
        assert_eq!(s["status"], "error");
        assert!(s["buffers"].as_array().unwrap().is_empty());
    }
    let (mut e, t) = create(SCAN, ScriptLimits::default());
    let r = e.advance(t, 1, true).unwrap();
    let req = r["events"][0]["detail"]["tool"]["request"]
        .as_u64()
        .unwrap();
    e.complete(t, req, None, None, Some("offline".into()))
        .unwrap();
    assert_eq!(e.snapshot(t).unwrap()["status"], "error");
}
#[test]
fn admission_and_collection_bounds() {
    let (mut e, t) = create("print(batch)", ScriptLimits::default());
    assert!(e.advance(t, 2049, true).is_err());
    assert!(e.advance(t, 0, true).is_err());
    assert!(e.prepare("import os", &[]).is_err());
    assert!(e.prepare("print(x)", &[]).is_err());
    for _ in 1..8 {
        e.create(0, &[2], ScriptLimits::default()).unwrap();
    }
    assert!(e.create(0, &[2], ScriptLimits::default()).is_err());
    for _ in 1..16 {
        e.prepare("print(1)", &[]).unwrap();
    }
    assert!(e.prepare("print(1)", &[]).is_err());
}

#[test]
fn fixture_import_checks_budget_before_allocation_and_cancel_frees_rows() {
    let mut l = ScriptLimits::default();
    l.limits.buffer_bytes = 16;
    let (mut e, t) = create(SCAN, l);
    let r = e.advance(t, 1, true).unwrap();
    let req = r["events"][0]["detail"]["tool"]["request"]
        .as_u64()
        .unwrap();
    let r = e
        .complete_fixture(
            t,
            req,
            usize::MAX,
            Quote {
                unit_price_cents: 7,
                quantity: 3,
            },
        )
        .unwrap();
    assert_eq!(r["snapshot"]["error"], "Budget");
    l = ScriptLimits::default();
    l.limits.chunk = 2;
    let (mut e, t) = create(SCAN, l);
    let r = e.advance(t, 1, true).unwrap();
    let req = r["events"][0]["detail"]["tool"]["request"]
        .as_u64()
        .unwrap();
    e.complete_fixture(
        t,
        req,
        5,
        Quote {
            unit_price_cents: 7,
            quantity: 3,
        },
    )
    .unwrap();
    let r = e.advance(t, 2048, true).unwrap();
    assert_eq!(
        r["events"].as_array().unwrap().last().unwrap()["kind"],
        "yield"
    );
    assert!(!r["snapshot"]["buffers"].as_array().unwrap().is_empty());
    let c = e.cancel(t).unwrap();
    assert_eq!(c["snapshot"]["buffers"], serde_json::json!([]));
    assert_eq!(c["snapshot"]["status"], "cancelled");
}

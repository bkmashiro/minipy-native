#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
use bytes::Bytes;
use minipy_native::{
    buffer::QuoteBuffer,
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    ir::Type,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits, ScriptOutcome},
    tool::{ArgumentSchema, JsonLeaf, Reply, ResultSchema, ToolArguments, ToolDeclaration},
};
use std::{
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::Notify;

fn declarations() -> Vec<ToolDeclaration> {
    vec![
        ToolDeclaration {
            id: 101,
            name: "fetch_state".into(),
            arguments: vec![ArgumentSchema::I64("key".into())],
            result: ResultSchema::JsonTyped(vec![
                (vec!["count".into()], JsonLeaf::I64),
                (vec!["nested".into(), "n".into()], JsonLeaf::I64),
                (vec!["status".into()], JsonLeaf::String),
            ]),
        },
        ToolDeclaration {
            id: 102,
            name: "emit".into(),
            arguments: vec![ArgumentSchema::String("text".into())],
            result: ResultSchema::I64,
        },
    ]
}
struct Host {
    data: Mutex<Option<Bytes>>,
    calls: Mutex<Vec<String>>,
    allow_emit: bool,
    delayed: bool,
    entered: Notify,
    finish: Notify,
    done: Notify,
}
impl Host {
    fn new(data: impl Into<Bytes>) -> Self {
        Self {
            data: Mutex::new(Some(data.into())),
            calls: Mutex::new(vec![]),
            allow_emit: true,
            delayed: false,
            entered: Notify::new(),
            finish: Notify::new(),
            done: Notify::new(),
        }
    }
}
impl Provider for Host {
    fn allows_tool(&self, id: u32) -> bool {
        id == 101 || id == 102 && self.allow_emit
    }
    async fn call_bound_tool(&self, id: u32, args: ToolArguments) -> Result<Reply, TaskError> {
        if id == 101 {
            let ToolArguments::Numeric(values, 1) = args else {
                panic!("numeric argument")
            };
            assert_eq!(values[0], 7);
            self.calls.lock().unwrap().push("fetch".into());
            return Ok(Reply::Json(self.data.lock().unwrap().take().unwrap()));
        }
        let ToolArguments::String(value) = args else {
            panic!("string argument")
        };
        self.entered.notify_one();
        if self.delayed {
            self.finish.notified().await;
        }
        self.calls.lock().unwrap().push(value.as_str().to_owned());
        self.done.notify_one();
        Ok(Reply::I64(9))
    }
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        Err(TaskError::Permission)
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
}
async fn run(
    native: bool,
    source: &str,
    host: Arc<Host>,
    limits: ScriptLimits,
    cancel: Cancellation,
) -> ScriptOutcome {
    let inputs = &[("request_id", Type::I64)];
    if native {
        runtime::execute_script(
            Rc::new(CodeUnit::compile_with_declarations(source, inputs, &declarations()).unwrap()),
            host,
            &[7],
            limits,
            cancel,
            None,
        )
        .await
    } else {
        interpreter::execute_script(
            Rc::new(
                InterpreterUnit::prepare_with_declarations(source, inputs, &declarations())
                    .unwrap(),
            ),
            host,
            &[7],
            limits,
            cancel,
            None,
        )
        .await
    }
}
const FETCH: &str = "d=tools.fetch_state(request_id)\n";

#[tokio::test]
async fn unchanged_cv01_and_unexecuted_branch() {
    let source = include_str!("../fixtures/cv/01_status_summary.py");
    for native in [false, true] {
        for (data, expected) in [
            (r#"{"status":"ready","count":3}"#, "ready: 3\n"),
            (r#"{"status":"waiting"}"#, "not ready\n"),
            (r#"{"status":"waiting","count":null}"#, "not ready\n"),
        ] {
            let host = Arc::new(Host::new(Bytes::copy_from_slice(data.as_bytes())));
            let out = run(
                native,
                source,
                host.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Ok(()));
            assert_eq!(out.stdout, expected);
            assert_eq!(*host.calls.lock().unwrap(), ["fetch"]);
        }
    }
}
#[tokio::test]
async fn integer_extrema_mixed_sinks_and_nested_view() {
    for native in [false, true] {
        for n in [i64::MIN, -1, 0, 1, i64::MAX] {
            let data = format!(r#"{{"count":{n},"nested":{{"n":{n}}},"status":"h\u00e9"}}"#);
            let source = format!(
                "{FETCH}v=d['nested']\nx=42\nb=True\nprint(f\"{{d['count']}}\")\nprint('[' + f\"{{d['status']}}/{{d['count']}}/{{x}}/{{b}}\" + ']')\nr=tools.emit(f\"{{v['n']}}\")\nprint(r)"
            );
            let host = Arc::new(Host::new(data));
            let out = run(
                native,
                &source,
                host.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Ok(()));
            assert_eq!(out.stdout, format!("{n}\n[hé/{n}/42/True]\n9\n"));
            assert_eq!(
                *host.calls.lock().unwrap(),
                ["fetch".to_owned(), n.to_string()]
            );
        }
    }
}
#[tokio::test]
async fn invalid_leaf_is_atomic_and_never_submits_tool() {
    for native in [false, true] {
        for (data, expected) in [
            (
                "{}",
                TaskError::Json("missing field or invalid JSON on accessed path"),
            ),
            (
                r#"{"count":null}"#,
                TaskError::Json("expected signed-i64 integer"),
            ),
            (
                r#"{"count":true}"#,
                TaskError::Json("expected signed-i64 integer"),
            ),
            (
                r#"{"count":"3"}"#,
                TaskError::Json("expected signed-i64 integer"),
            ),
            (
                r#"{"count":1.5}"#,
                TaskError::Json("expected signed-i64 integer"),
            ),
            (
                r#"{"count":9223372036854775808}"#,
                TaskError::Json("expected signed-i64 integer"),
            ),
            (
                r#"{"count":-9223372036854775809}"#,
                TaskError::Json("expected signed-i64 integer"),
            ),
        ] {
            for sink in [
                "print(f\"prefix {d['count']} tail\")",
                "r=tools.emit(f\"prefix {d['count']} tail\")",
            ] {
                let host = Arc::new(Host::new(Bytes::copy_from_slice(data.as_bytes())));
                let source = format!("print(1)\n{FETCH}{sink}\nr=tools.emit('unreached')");
                let out = run(
                    native,
                    &source,
                    host.clone(),
                    ScriptLimits::default(),
                    Cancellation::default(),
                )
                .await;
                assert_eq!(out.status, Err(expected.clone()), "{native} {data} {sink}");
                assert_eq!(out.stdout, "1\n");
                assert_eq!(*host.calls.lock().unwrap(), ["fetch"]);
            }
        }
    }
}
#[tokio::test]
async fn left_to_right_scan_type_size_and_authority_priority() {
    for native in [false, true] {
        for (expr, scan, expected) in [
            ("f\"{d['status']}/{d['count']}\"", 0, TaskError::Budget),
            (
                "f\"{d['count']}/{d['status']}\"",
                0,
                TaskError::Json("expected signed-i64 integer"),
            ),
            (
                "f\"{d['status']}/{d['count']}\"",
                100,
                TaskError::Json("expected UTF-8 string"),
            ),
            (
                "f\"{d['count']}/{d['status']}\"",
                100,
                TaskError::Json("expected signed-i64 integer"),
            ),
        ] {
            for sink in [format!("print({expr})"), format!("r=tools.emit({expr})")] {
                let mut h = Host::new(Bytes::from_static(br#"{"status":null,"count":null}"#));
                h.allow_emit = false;
                let host = Arc::new(h);
                let out = run(
                    native,
                    &format!("{FETCH}{sink}"),
                    host.clone(),
                    ScriptLimits {
                        allow_print: false,
                        string_scan_bytes: scan,
                        ..ScriptLimits::default()
                    },
                    Cancellation::default(),
                )
                .await;
                assert_eq!(out.status, Err(expected.clone()));
                assert_eq!(out.stdout, "");
                assert_eq!(*host.calls.lock().unwrap(), ["fetch"]);
            }
        }
        // i64 reads have the existing integer cache contract, no string scan charge.
        for (scan, expected) in [(2, Err(TaskError::Budget)), (3, Ok(()))] {
            let host = Arc::new(Host::new(Bytes::from_static(
                br#"{"count":7,"status":"x"}"#,
            )));
            let source = format!(
                "{FETCH}print(f\"{{d['count']}}/{{d['status']}}\")\nprint(f\"{{d['count']}}\")"
            );
            let out = run(
                native,
                &source,
                host,
                ScriptLimits {
                    string_scan_bytes: scan,
                    ..ScriptLimits::default()
                },
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, expected);
            assert_eq!(out.stdout, if scan == 3 { "7/x\n7\n" } else { "" });
        }
        // Whole-line stdout cap checked after evaluating fields.
        for (data, expected) in [
            (
                r#"{"count":null}"#,
                TaskError::Json("expected signed-i64 integer"),
            ),
            (r#"{"count":7}"#, TaskError::Budget),
        ] {
            let out = run(
                native,
                &format!("print(1)\n{FETCH}print(f\"{{d['count']}}\")"),
                Arc::new(Host::new(Bytes::copy_from_slice(data.as_bytes()))),
                ScriptLimits {
                    stdout_bytes: 2,
                    ..ScriptLimits::default()
                },
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Err(expected));
            assert_eq!(out.stdout, "1\n");
        }
        // A too-large earlier string piece fails before a later invalid integer.
        let data = format!(r#"{{"count":null,"status":"{}"}}"#, "x".repeat(40_000));
        for sink in [
            "print(f\"{d['status']}{d['status']}{d['count']}\")",
            "r=tools.emit(f\"{d['status']}{d['status']}{d['count']}\")",
        ] {
            let host = Arc::new(Host::new(data.clone()));
            let out = run(
                native,
                &format!("{FETCH}{sink}"),
                host.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Err(TaskError::Budget));
            assert_eq!(out.stdout, "");
            assert_eq!(*host.calls.lock().unwrap(), ["fetch"]);
        }
    }
}

#[tokio::test]
async fn evaluated_integer_does_not_bypass_authority_or_call_budget() {
    for native in [false, true] {
        for (sink, allow_print, allow_emit, calls, expected) in [
            (
                "print(f\"{d['count']}\")",
                false,
                true,
                2,
                TaskError::Permission,
            ),
            (
                "r=tools.emit(f\"{d['count']}\")",
                true,
                false,
                2,
                TaskError::Permission,
            ),
            (
                "r=tools.emit(f\"{d['count']}\")",
                true,
                true,
                1,
                TaskError::Budget,
            ),
        ] {
            let mut h = Host::new(Bytes::from_static(br#"{"count":7}"#));
            h.allow_emit = allow_emit;
            let host = Arc::new(h);
            let out = run(
                native,
                &format!("{FETCH}{sink}"),
                host.clone(),
                ScriptLimits {
                    allow_print,
                    tool_calls: calls,
                    string_scan_bytes: 0,
                    ..ScriptLimits::default()
                },
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Err(expected));
            assert_eq!(out.stdout, "");
            assert_eq!(*host.calls.lock().unwrap(), ["fetch"]);
        }
    }
}

struct Observed {
    data: Vec<u8>,
    drops: Arc<AtomicUsize>,
}
impl AsRef<[u8]> for Observed {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}
impl Drop for Observed {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
#[tokio::test]
async fn root_liveness_across_wait_and_detached_argument_after_cancel() {
    for native in [false, true] {
        for future_read in [false, true] {
            for cancelled in [false, true] {
                let drops = Arc::new(AtomicUsize::new(0));
                let mut h = Host::new(Bytes::from_owner(Observed {
                    data: br#"{"count":-9223372036854775808}"#.to_vec(),
                    drops: drops.clone(),
                }));
                h.delayed = true;
                let host = Arc::new(h);
                let source = format!(
                    "{FETCH}r=tools.emit(f\"{{d['count']}}\")\n{}",
                    if future_read {
                        "print(f\"{d['count']}\")"
                    } else {
                        "print(r)"
                    }
                );
                let cancel = Cancellation::default();
                let task = run(
                    native,
                    &source,
                    host.clone(),
                    ScriptLimits::default(),
                    cancel.clone(),
                );
                tokio::pin!(task);
                tokio::select! { _ = host.entered.notified() => {}, out = &mut task => panic!("early {:?}", out.status) }
                assert_eq!(drops.load(Ordering::SeqCst), usize::from(!future_read));
                if cancelled {
                    cancel.cancel();
                    let out = task.await;
                    assert_eq!(out.status, Err(TaskError::Cancelled));
                    assert_eq!(out.stdout, "");
                    assert_eq!(drops.load(Ordering::SeqCst), 1);
                    host.finish.notify_one();
                    host.done.notified().await;
                } else {
                    host.finish.notify_one();
                    let out = task.await;
                    assert_eq!(out.status, Ok(()));
                    assert_eq!(
                        out.stdout,
                        if future_read {
                            "-9223372036854775808\n"
                        } else {
                            "9\n"
                        }
                    );
                }
                assert_eq!(
                    *host.calls.lock().unwrap(),
                    ["fetch", "-9223372036854775808"]
                );
                assert_eq!(drops.load(Ordering::SeqCst), 1);
            }
        }
    }
}
#[test]
fn admission_stays_limited_to_declared_leaves_and_existing_format_subset() {
    for expr in [
        "'x' + d['count']",
        "f\"{d['count']:04d}\"",
        "f\"{d['count']!r}\"",
        "f\"{d['count'] + 1}\"",
        "f\"{d['nested']}\"",
        "f\"{d['missing']}\"",
        "f\"{d['count'].real}\"",
    ] {
        let source = format!("{FETCH}print({expr})");
        assert!(
            CodeUnit::compile_with_declarations(
                &source,
                &[("request_id", Type::I64)],
                &declarations()
            )
            .is_err(),
            "{expr}"
        );
        assert!(
            InterpreterUnit::prepare_with_declarations(
                &source,
                &[("request_id", Type::I64)],
                &declarations()
            )
            .is_err(),
            "{expr}"
        );
    }
}

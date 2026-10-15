#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
use bytes::Bytes;
use minipy_native::{
    buffer::QuoteBuffer,
    codegen::CodeUnit,
    error::TaskError,
    frontend,
    interpreter::{self, InterpreterUnit},
    ir::Type,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits, ScriptOutcome},
    tool::{Reply, ResultSchema, ToolSchema},
};
use std::{
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

#[tokio::test]
async fn multiply_add_preserves_fallible_operand_order() {
    for native in [false, true] {
        for (expression, expected) in [
            (
                "9223372036854775807*2+p['data']['count']",
                TaskError::Overflow,
            ),
            (
                "p['data']['count']+9223372036854775807*2",
                TaskError::Json("expected signed-i64 integer"),
            ),
            (
                "(9223372036854775807*2)+(p['data']['count']+1)",
                TaskError::Overflow,
            ),
        ] {
            let provider = Arc::new(P::new(br#"{"data":{"count":null}}"#));
            let source = format!(
                "p=tools.fetch(batch_id)\nprint(17)\nx={expression}\nreceipt=tools.publish(batch_id,x)"
            );
            let out = run(
                native,
                &source,
                provider.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Err(expected), "{source}");
            assert_eq!(out.stdout, "17\n");
            assert_eq!(provider.calls.lock().unwrap().len(), 1);
        }
    }
}
#[tokio::test]
async fn direct_scalar_tool_print_matches_explicit_binding_and_bounds() {
    for native in [false, true] {
        for direct in [false, true] {
            let suffix = if direct {
                "print(tools.publish(total=p[\"data\"][\"count\"] * 2, batch_id=batch_id))\n"
            } else {
                "receipt = tools.publish(total=p[\"data\"][\"count\"] * 2, batch_id=batch_id)\nprint(receipt)\n"
            };
            let source = format!("print(1)\np = tools.fetch(batch_id)\n{suffix}");
            for (options, expected) in [
                (ScriptLimits::default(), Ok(())),
                (
                    ScriptLimits {
                        stdout_bytes: 2,
                        ..ScriptLimits::default()
                    },
                    Err(TaskError::Budget),
                ),
                (
                    ScriptLimits {
                        tool_calls: 1,
                        ..ScriptLimits::default()
                    },
                    Err(TaskError::Budget),
                ),
            ] {
                let p = Arc::new(P::new(br#"{"data":{"count":10}}"#));
                let r = run(native, &source, p.clone(), options, Cancellation::default()).await;
                assert_eq!(r.status, expected);
                assert_eq!(r.stdout, if expected.is_ok() { "1\n27\n" } else { "1\n" });
                assert_eq!(p.live.load(Ordering::SeqCst), 0);
                assert_eq!(
                    p.calls.lock().unwrap().len(),
                    if options.tool_calls == 1 { 1 } else { 2 }
                );
                if options.tool_calls > 1 {
                    assert_eq!(*p.wait_live.lock().unwrap(), vec![0]);
                }
            }
        }
        let mut p = P::new(b"{}");
        p.denied = true;
        let p = Arc::new(p);
        let r = run(
            native,
            "print(1)\nprint(tools.publish(total=3, batch_id=batch_id))",
            p.clone(),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(r.status, Err(TaskError::Permission));
        assert_eq!(r.stdout, "1\n");
        assert!(p.calls.lock().unwrap().is_empty());
        let p = Arc::new(P::new(b"{}"));
        let r = run(
            native,
            "print(tools.publish(batch_id, 3))",
            p.clone(),
            ScriptLimits {
                allow_print: false,
                ..ScriptLimits::default()
            },
            Cancellation::default(),
        )
        .await;
        assert_eq!(r.status, Err(TaskError::Permission));
        assert_eq!(r.stdout, "");
        assert_eq!(
            p.calls.lock().unwrap().len(),
            1,
            "Tool argument to print is evaluated before print permission"
        );
        let p = Arc::new(P::new(b"{}"));
        let r = run(
            native,
            "print(1)\nprint(tools.publish(batch_id, 9223372036854775807))",
            p.clone(),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(
            r.status,
            Err(TaskError::Overflow),
            "provider failure is propagated"
        );
        assert_eq!(r.stdout, "1\n");
        assert_eq!(p.calls.lock().unwrap().len(), 1);
        let p = Arc::new(P::new(br#"{"data":{"count":null}}"#));
        let r = run(native, "p = tools.fetch(batch_id)\nprint(1)\nprint(tools.publish(total=9223372036854775807+1, batch_id=p[\"data\"][\"count\"]))", p.clone(), ScriptLimits::default(), Cancellation::default()).await;
        assert_eq!(r.status, Err(TaskError::Overflow));
        assert_eq!(r.stdout, "1\n");
        assert_eq!(p.calls.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn direct_tool_print_suspends_and_cancellation_preserves_prefix() {
    for native in [false, true] {
        let mut p = P::new(b"{}");
        p.hold_publish = true;
        let p = Arc::new(p);
        let cancel = Cancellation::default();
        let task = run(
            native,
            "print(1)\nprint(tools.publish(batch_id, 3))\nprint(2)",
            p.clone(),
            ScriptLimits::default(),
            cancel.clone(),
        );
        tokio::pin!(task);
        tokio::select! {r=&mut task=>panic!("unexpected {:?}",r.status),_=p.loaded.notified()=>{}}
        cancel.cancel();
        let r = task.await;
        assert_eq!(r.status, Err(TaskError::Cancelled));
        assert_eq!(r.stdout, "1\n");
        assert_eq!(p.calls.lock().unwrap().len(), 1);
        p.proceed.notify_one();
        tokio::task::yield_now().await;
    }
}

#[test]
fn direct_tool_print_rejects_json_nested_calls_and_loop_effects() {
    for source in [
        "tools.fetch(batch_id)",
        "tools.publish(**kwargs)",
        "tools.unknown(batch_id)",
        "q = tools.load_quotes(batch_id)\nfor row in q:\n    tools.publish(batch_id, 2)",
        "print(tools.fetch(batch_id))",
        "print(tools.publish(batch_id, 2) + 1)",
        "print(tools.publish(batch_id, tools.publish(batch_id, 2)))",
        "print(tools.publish(batch_id, total=2, batch_id=3))",
        "print(tools.publish(other=3, total=2))",
        "print(tools.publish(batch_id))",
        "print(tools.publish(**batch_id))",
        "print(tools.publish(batch_id, True))",
        "q = tools.load_quotes(batch_id)\nfor row in q:\n    print(tools.publish(batch_id, 2))",
    ] {
        assert!(
            frontend::parse_with_tools(source, &[("batch_id", Type::I64)], &schema()).is_err(),
            "{source}"
        );
    }
}

#[tokio::test]
async fn standalone_scalar_tool_call_executes_and_discards_result() {
    for native in [false, true] {
        for source in [
            "tools.publish(batch_id, total=3)\nprint(7)",
            "await tools.publish(total=3, batch_id=batch_id)\nprint(7)",
        ] {
            let p = Arc::new(P::new(b"{}"));
            let r = run(
                native,
                source,
                p.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(r.status, Ok(()));
            assert_eq!(r.stdout, "7\n");
            let calls = p.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, 102);
            assert_eq!(calls[0].1, [7, 3]);
        }

        let denied = Arc::new({
            let mut p = P::new(b"{}");
            p.denied = true;
            p
        });
        let r = run(
            native,
            "print(1)\ntools.publish(batch_id, 3)\nprint(2)",
            denied.clone(),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(r.status, Err(TaskError::Permission));
        assert_eq!(r.stdout, "1\n");
        assert!(denied.calls.lock().unwrap().is_empty());

        let limited = Arc::new(P::new(b"{}"));
        let r = run(
            native,
            "tools.publish(batch_id, 3)",
            limited.clone(),
            ScriptLimits {
                tool_calls: 0,
                ..ScriptLimits::default()
            },
            Cancellation::default(),
        )
        .await;
        assert_eq!(r.status, Err(TaskError::Budget));
        assert!(limited.calls.lock().unwrap().is_empty());

        let overflow = Arc::new(P::new(b"{}"));
        let r = run(
            native,
            "tools.publish(batch_id, 9223372036854775807 + 1)",
            overflow.clone(),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(r.status, Err(TaskError::Overflow));
        assert!(overflow.calls.lock().unwrap().is_empty());

        let failure = Arc::new(P::new(b"{}"));
        let r = run(
            native,
            "print(1)\ntools.publish(batch_id, 9223372036854775807)",
            failure.clone(),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(r.status, Err(TaskError::Overflow));
        assert_eq!(r.stdout, "1\n");
        assert_eq!(failure.calls.lock().unwrap().len(), 1);

        let mut pending_provider = P::new(b"{}");
        pending_provider.hold_publish = true;
        let pending_provider = Arc::new(pending_provider);
        let cancel = Cancellation::default();
        let task = run(
            native,
            "print(1)\ntools.publish(batch_id, 3)\nprint(2)",
            pending_provider.clone(),
            ScriptLimits::default(),
            cancel.clone(),
        );
        tokio::pin!(task);
        tokio::select! {
            r = &mut task => panic!("unexpected completion: {:?}", r.status),
            _ = pending_provider.loaded.notified() => {}
        }
        cancel.cancel();
        let r = task.await;
        assert_eq!(r.status, Err(TaskError::Cancelled));
        assert_eq!(r.stdout, "1\n");
        pending_provider.proceed.notify_one();
        tokio::task::yield_now().await;
    }
}

fn schema() -> Vec<ToolSchema> {
    vec![
        ToolSchema {
            id: 101,
            name: "fetch".into(),
            arguments: vec!["batch_id".into()],
            result: ResultSchema::Json(vec![
                vec!["data".into(), "count".into()],
                vec!["data".into(), "bad".into()],
                vec!["esc".into()],
            ]),
        },
        ToolSchema {
            id: 102,
            name: "publish".into(),
            arguments: vec!["batch_id".into(), "total".into()],
            result: ResultSchema::I64,
        },
    ]
}
struct Owner {
    data: Box<[u8]>,
    live: Arc<AtomicUsize>,
}
impl AsRef<[u8]> for Owner {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.live.fetch_sub(1, Ordering::SeqCst);
    }
}
struct P {
    text: &'static [u8],
    live: Arc<AtomicUsize>,
    calls: Mutex<Vec<(u32, [i64; 2], usize)>>,
    wait_live: Mutex<Vec<usize>>,
    delay: Duration,
    denied: bool,
    loaded: tokio::sync::Notify,
    proceed: tokio::sync::Notify,
    hold_publish: bool,
}
impl P {
    fn new(text: &'static [u8]) -> Self {
        Self {
            text,
            live: Arc::new(AtomicUsize::new(0)),
            calls: Mutex::new(vec![]),
            wait_live: Mutex::new(vec![]),
            delay: Duration::ZERO,
            denied: false,
            loaded: tokio::sync::Notify::new(),
            proceed: tokio::sync::Notify::new(),
            hold_publish: false,
        }
    }
}
impl Provider for P {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        Err(TaskError::Permission)
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
    fn allows_tool(&self, id: u32) -> bool {
        !self.denied && [101, 102].contains(&id)
    }
    async fn call_tool(&self, id: u32, args: [i64; 2], count: usize) -> Result<Reply, TaskError> {
        self.calls.lock().unwrap().push((id, args, count));
        if id == 101 {
            self.live.fetch_add(1, Ordering::SeqCst);
            let owner = Owner {
                data: self.text.to_vec().into_boxed_slice(),
                live: self.live.clone(),
            };
            let ptr = owner.data.as_ptr() as usize;
            let bytes = Bytes::from_owner(owner);
            assert_eq!(
                ptr,
                bytes.as_ptr() as usize,
                "host-to-task transfer must not copy payload"
            );
            self.loaded.notify_one();
            tokio::time::sleep(self.delay).await;
            Ok(Reply::Json(bytes))
        } else {
            self.wait_live
                .lock()
                .unwrap()
                .push(self.live.load(Ordering::SeqCst));
            self.loaded.notify_one();
            if self.hold_publish {
                self.proceed.notified().await;
            }
            Ok(Reply::I64(
                args[0].checked_add(args[1]).ok_or(TaskError::Overflow)?,
            ))
        }
    }
}
async fn run(
    native: bool,
    s: &str,
    p: Arc<P>,
    options: ScriptLimits,
    cancel: Cancellation,
) -> ScriptOutcome {
    let inputs = &[("batch_id", Type::I64)];
    if native {
        runtime::execute_script(
            Rc::new(CodeUnit::compile_with_tools(s, inputs, &schema()).unwrap()),
            p,
            &[7],
            options,
            cancel,
            None,
        )
        .await
    } else {
        interpreter::execute_script(
            Rc::new(InterpreterUnit::prepare_with_tools(s, inputs, &schema()).unwrap()),
            p,
            &[7],
            options,
            cancel,
            None,
        )
        .await
    }
}
#[tokio::test]
async fn source_binding_and_real_backends_share_results() {
    let source = "payload = tools.fetch(batch_id)\ndata = payload[\"data\"]\nn = data[\"count\"] + data[\"count\"]\nreceipt = tools.publish(total=n, batch_id=batch_id)\nprint(receipt)\n";
    let program =
        frontend::parse_with_tools(source, &[("batch_id", Type::I64)], &schema()).unwrap();
    assert_eq!(program.json.calls[0].id, 101);
    assert_eq!(program.json.calls[1].id, 102);
    assert_eq!(
        program.json.paths.len(),
        2,
        "prefix and repeated leaf share slots"
    );
    for native in [false, true] {
        let p = Arc::new(P::new(br#"{"data":{"count":10,"bad":null}}"#));
        let r = run(
            native,
            source,
            p.clone(),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(r.status, Ok(()));
        assert_eq!(r.stdout, "27\n");
        assert_eq!(
            *p.calls.lock().unwrap(),
            vec![(101, [7, 0], 1), (102, [7, 20], 2)]
        );
        assert_eq!(*p.wait_live.lock().unwrap(), vec![0]);
        assert_eq!(p.live.load(Ordering::SeqCst), 0);
    }
}
#[tokio::test]
async fn subview_keeps_root_across_actual_wait_then_releases() {
    for native in [false, true] {
        let mut p = P::new(br#"{"data":{"count":10}}"#);
        p.hold_publish = true;
        let p = Arc::new(p);
        let source = "payload = tools.fetch(batch_id)\ndata = payload[\"data\"]\nr = tools.publish(batch_id, 3)\nprint(data[\"count\"] + r)\ns = tools.publish(batch_id, 4)\n";
        let task = run(
            native,
            source,
            p.clone(),
            ScriptLimits::default(),
            Cancellation::default(),
        );
        tokio::pin!(task);
        // Both fetch and publish signal; wait until the second operation has started.
        loop {
            tokio::select! {r=&mut task=>panic!("completed early {:?}",r.status),_=p.loaded.notified()=>{if p.calls.lock().unwrap().len()==2 {break;}}}
        }
        assert_eq!(p.live.load(Ordering::SeqCst), 1);
        assert_eq!(*p.wait_live.lock().unwrap(), vec![1]);
        p.proceed.notify_one();
        loop {
            tokio::select! {r=&mut task=>panic!("completed before final publish {:?}",r.status),_=p.loaded.notified()=>{if p.calls.lock().unwrap().len()==3 {break;}}}
        }
        assert_eq!(p.live.load(Ordering::SeqCst), 0);
        assert_eq!(*p.wait_live.lock().unwrap(), vec![1, 0]);
        p.proceed.notify_one();
        let r = task.await;
        assert_eq!(r.status, Ok(()));
        assert_eq!(r.stdout, "20\n");
    }
}
#[tokio::test]
async fn cancellation_detached_late_json_response_drops_owner() {
    for native in [false, true] {
        let mut p = P::new(br#"{"data":{"count":10}}"#);
        p.delay = Duration::from_millis(40);
        let p = Arc::new(p);
        let cancel = Cancellation::default();
        let task = run(
            native,
            "p = tools.fetch(batch_id)\nprint(p[\"data\"][\"count\"])\n",
            p.clone(),
            ScriptLimits::default(),
            cancel.clone(),
        );
        tokio::pin!(task);
        tokio::select! {r=&mut task=>panic!("unexpected {:?}",r.status),_=p.loaded.notified()=>{}}
        cancel.cancel();
        let r = task.await;
        assert_eq!(r.status, Err(TaskError::Cancelled));
        assert_eq!(r.stdout, "");
        assert_eq!(p.live.load(Ordering::SeqCst), 1);
        tokio::time::sleep(Duration::from_millis(55)).await;
        assert_eq!(p.live.load(Ordering::SeqCst), 0);
        assert_eq!(p.calls.lock().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn cancellation_with_live_subview_drops_task_owner_not_operation() {
    for native in [false, true] {
        let mut p = P::new(br#"{"data":{"count":10}}"#);
        p.hold_publish = true;
        let p = Arc::new(p);
        let cancel = Cancellation::default();
        let task = run(
            native,
            "p = tools.fetch(batch_id)\nd = p[\"data\"]\nprint(1)\nr = tools.publish(batch_id, 2)\nprint(d[\"count\"])\n",
            p.clone(),
            ScriptLimits::default(),
            cancel.clone(),
        );
        tokio::pin!(task);
        loop {
            tokio::select! {r=&mut task=>panic!("early {:?}",r.status),_=p.loaded.notified()=>{if p.calls.lock().unwrap().len()==2 {break;}}}
        }
        assert_eq!(p.live.load(Ordering::SeqCst), 1);
        cancel.cancel();
        let r = task.await;
        assert_eq!(r.status, Err(TaskError::Cancelled));
        assert_eq!(r.stdout, "1\n");
        assert_eq!(p.live.load(Ordering::SeqCst), 0);
        p.proceed.notify_one();
    }
}
#[tokio::test]
async fn accessed_errors_short_circuit_and_left_to_right_are_preserved() {
    for native in [false, true] {
        for bytes in [
            br#"{"data":{}}"#.as_slice(),
            br#"{"data":{"count":null}}"#,
            br#"{"data":{"count":"10"}}"#,
            br#"{"data":{"count":1.5}}"#,
            br#"{"data":{"count":true}}"#,
            br#"{"data":{"count":9223372036854775808}}"#,
            br#"{"data":{"count":-9223372036854775809}}"#,
        ] {
            let p = Arc::new(P::new(bytes));
            let r=run(native,"p = tools.fetch(batch_id)\nprint(1)\nprint(p[\"data\"][\"count\"])\nr = tools.publish(batch_id, 2)\n",p.clone(),ScriptLimits::default(),Cancellation::default()).await;
            assert!(matches!(r.status, Err(TaskError::Json(_))));
            assert_eq!(r.stdout, "1\n");
            assert_eq!(p.calls.lock().unwrap().len(), 1);
            assert_eq!(p.live.load(Ordering::SeqCst), 0);
        }
        let p = Arc::new(P::new(br#"{"data":{"count":10,"bad":null}}"#));
        let r=run(native,"p = tools.fetch(batch_id)\nprint(False and p[\"data\"][\"bad\"] > 0)\nprint(True or p[\"data\"][\"bad\"] > 0)\nprint(p[\"data\"][\"count\"] + 1)\n",p,ScriptLimits::default(),Cancellation::default()).await;
        assert_eq!(r.status, Ok(()));
        assert_eq!(r.stdout, "False\nTrue\n11\n");
        let p = Arc::new(P::new(br#"{"data":{"count":null}}"#));
        let r=run(native,"p = tools.fetch(batch_id)\nr = tools.publish(total=9223372036854775807+1, batch_id=p[\"data\"][\"count\"])\n",p.clone(),ScriptLimits::default(),Cancellation::default()).await;
        assert_eq!(
            r.status,
            Err(TaskError::Overflow),
            "named arguments evaluate source order, not ABI order"
        );
        assert_eq!(p.calls.lock().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn duplicate_escaped_keys_extremes_and_unvisited_suffix_contract() {
    for native in [false, true] {
        for (bytes, expected) in [
            (br#"{"data":{"count":3,"count":99}}"#.as_slice(), "3\n"),
            (
                br#"{"data":{"co\u0075nt":-9223372036854775808}}"#,
                "-9223372036854775808\n",
            ),
            (
                br#"{"data":{"count":9223372036854775807}}"#,
                "9223372036854775807\n",
            ),
        ] {
            let r = run(
                native,
                "p = tools.fetch(batch_id)\nprint(p[\"data\"][\"count\"])\n",
                Arc::new(P::new(bytes)),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(r.status, Ok(()));
            assert_eq!(r.stdout, expected);
        }
        let r = run(
            native,
            "p = tools.fetch(batch_id)\nprint(False and p[\"data\"][\"bad\"] > 0)\n",
            Arc::new(P::new(br#"{"data": {"bad": tru"#)),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(r.status, Ok(()));
        assert_eq!(
            r.stdout, "False\n",
            "lazy import is not full-document validation"
        );
    }
}
#[tokio::test]
async fn task_tool_permissions_counts_and_buffer_cap() {
    for native in [false, true] {
        let source = "p = tools.fetch(batch_id)\nprint(p[\"data\"][\"count\"])\n";
        let mut p = P::new(br#"{"data":{"count":10}}"#);
        p.denied = true;
        let p = Arc::new(p);
        let r = run(
            native,
            source,
            p.clone(),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(r.status, Err(TaskError::Permission));
        assert!(p.calls.lock().unwrap().is_empty());
        let p = Arc::new(P::new(br#"{"data":{"count":10}}"#));
        let r = run(
            native,
            source,
            p.clone(),
            ScriptLimits {
                tool_calls: 0,
                ..ScriptLimits::default()
            },
            Cancellation::default(),
        )
        .await;
        assert_eq!(r.status, Err(TaskError::Budget));
        assert!(p.calls.lock().unwrap().is_empty());
        let p = Arc::new(P::new(br#"{"data":{"count":10}}"#));
        let mut limits = ScriptLimits::default();
        limits.limits.buffer_bytes = 1;
        let r = run(native, source, p.clone(), limits, Cancellation::default()).await;
        assert_eq!(r.status, Err(TaskError::Budget));
        assert_eq!(p.live.load(Ordering::SeqCst), 0);
    }
}
#[test]
fn schema_and_binding_reject_unsupported_without_runtime() {
    for source in [
        "p = tools.fetch()",
        "p = tools.fetch(True)",
        "p = tools.fetch(batch_id, 2)",
        "p = tools.fetch(batch_id, batch_id=2)",
        "p = tools.fetch(other=2)",
        "p = tools.unknown(batch_id)",
        "p = tools.fetch(batch_id)\nprint(p[\"unknown\"])",
        "if True:\n    p = tools.fetch(batch_id)",
        "p = tools.fetch(batch_id)\np = tools.fetch(batch_id)",
        "p = tools.fetch(batch_id)\nx = p[batch_id]",
        "p = tools.fetch(batch_id)\nprint(p)",
    ] {
        assert!(
            frontend::parse_with_tools(source, &[("batch_id", Type::I64)], &schema()).is_err(),
            "{source}"
        );
    }
    let mut tools = schema();
    tools[1].id = 101;
    assert!(ToolSchema::validate_all(&tools).is_err());
}

#[tokio::test]
async fn branch_last_use_alias_error_and_multiple_owner_budget() {
    for native in [false, true] {
        let p = Arc::new(P::new(br#"{"data":{"count":10}}"#));
        let r = run(native, "p = tools.fetch(batch_id)\nif p[\"data\"][\"count\"] > 0:\n    r = tools.publish(batch_id, 3)\nprint(1)\n", p.clone(), ScriptLimits::default(), Cancellation::default()).await;
        assert_eq!(r.status, Ok(()));
        assert_eq!(*p.wait_live.lock().unwrap(), vec![0]);
        let p = Arc::new(P::new(br#"{}"#));
        let r = run(
            native,
            "p = tools.fetch(batch_id)\nd = p[\"data\"]\nprint(1)\n",
            p,
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert!(matches!(r.status, Err(TaskError::Json(_))));
        assert_eq!(r.stdout, "");
        let p = Arc::new(P::new(br#"{"data":{"count":10}}"#));
        let mut options = ScriptLimits::default();
        options.limits.buffer_bytes = p.text.len();
        let r = run(native,"p = tools.fetch(batch_id)\nq = tools.fetch(batch_id)\nprint(p[\"data\"][\"count\"] + q[\"data\"][\"count\"])\n",p.clone(),options,Cancellation::default()).await;
        assert_eq!(r.status, Err(TaskError::Budget));
        assert_eq!(p.live.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn malformed_accessed_content_is_task_error_not_partial_success() {
    for native in [false, true] {
        let lazy = run(
            native,
            "p = tools.fetch(batch_id)\nprint(p[\"data\"][\"count\"])\n",
            Arc::new(P::new(b"{\"data\":{\"count\":10}")),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(lazy.status, Ok(()));
        assert_eq!(
            lazy.stdout, "10\n",
            "missing outer terminator outside selected subtree is not fully validated"
        );
        for bytes in [
            b"{\"data\":{\"count\":tru}}".as_slice(),
            b"{\"data\":{\"count\":10",
            b"{\"data\":{\"count\":\xff}}",
        ] {
            let r = run(
                native,
                "p = tools.fetch(batch_id)\nprint(p[\"data\"][\"count\"])\n",
                Arc::new(P::new(bytes)),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert!(
                matches!(r.status, Err(TaskError::Json(_))),
                "{:?}: {:?}",
                bytes,
                r.status
            );
        }
    }
}

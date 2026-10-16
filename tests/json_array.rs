#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
use bytes::Bytes;
use minipy_native::{
    buffer::QuoteBuffer,
    codegen::CodeUnit,
    error::TaskError,
    frontend,
    interpreter::{self, InterpreterUnit},
    ir::{AdmissionKind, Type},
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits, ScriptOutcome},
    tool::{
        ArgumentSchema, ObjectArraySchema, Reply, ResultSchema, ToolArguments, ToolDeclaration,
    },
};
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

const CV: &str = include_str!("../fixtures/cv/04_first_matching_result.py");

#[tokio::test]
async fn dotted_array_and_row_are_the_same_bound_program() {
    let indexed = "data=tools.search(request_id)\ntotal=0\nfor row in data['results']:\n    total += row['score']\n    if row['id'] == 9:\n        break\nprint(total)";
    let dotted = indexed
        .replace("data['results']", "data.results")
        .replace("row['score']", "row.score")
        .replace("row['id']", "row.id");
    let inputs = &[("request_id", Type::I64)];
    assert_eq!(
        format!(
            "{:?}",
            frontend::parse_with_declarations(indexed, inputs, &declarations()).unwrap()
        ),
        format!(
            "{:?}",
            frontend::parse_with_declarations(&dotted, inputs, &declarations()).unwrap()
        )
    );
    for native in [false, true] {
        for source in [indexed, &dotted] {
            let host = Arc::new(Host::new(
                r#"{"results":[{"score":3,"id":1},{"score":4,"id":9},null]}"#,
            ));
            let out = run(
                native,
                source,
                host,
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Ok(()));
            assert_eq!(out.stdout, "7\n");
        }
    }
    for field in [
        "row.unknown",
        "row['unknown']",
        "row.score.real",
        "row[request_id]",
    ] {
        let source =
            format!("data=tools.search(request_id)\nfor row in data.results:\n    print({field})");
        assert!(CodeUnit::compile_with_declarations(&source, inputs, &declarations()).is_err());
        assert!(
            InterpreterUnit::prepare_with_declarations(&source, inputs, &declarations()).is_err()
        );
    }
}

fn declarations() -> Vec<ToolDeclaration> {
    vec![
        ToolDeclaration {
            id: 101,
            name: "search".into(),
            arguments: vec![ArgumentSchema::I64("request_id".into())],
            result: ResultSchema::JsonObjectArray(Box::new(ObjectArraySchema {
                path: vec!["results".into()],
                i64_fields: vec!["score".into(), "id".into()],
            })),
        },
        ToolDeclaration {
            id: 102,
            name: "checkpoint".into(),
            arguments: vec![ArgumentSchema::I64("request_id".into())],
            result: ResultSchema::I64,
        },
    ]
}
struct Host {
    data: Mutex<Option<Bytes>>,
    calls: Mutex<Vec<u32>>,
    wait: bool,
    entered: tokio::sync::Notify,
    finish: tokio::sync::Notify,
}
impl Host {
    fn new(data: impl Into<Bytes>) -> Self {
        Self {
            data: Mutex::new(Some(data.into())),
            calls: Mutex::new(vec![]),
            wait: false,
            entered: tokio::sync::Notify::new(),
            finish: tokio::sync::Notify::new(),
        }
    }
}
impl Provider for Host {
    fn allows_tool(&self, _: u32) -> bool {
        true
    }
    async fn call_bound_tool(&self, id: u32, args: ToolArguments) -> Result<Reply, TaskError> {
        assert!(matches!(args, ToolArguments::Numeric([7, _], 1)));
        self.calls.lock().unwrap().push(id);
        if id == 101 {
            Ok(Reply::Json(self.data.lock().unwrap().take().unwrap()))
        } else {
            self.entered.notify_one();
            if self.wait {
                self.finish.notified().await;
            } else {
                tokio::task::yield_now().await;
            }
            Ok(Reply::I64(9))
        }
    }
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        QuoteBuffer::from_quotes(vec![minipy_native::buffer::Quote {
            unit_price_cents: 2,
            quantity: 1,
        }])
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
    run_trace(native, source, host, limits, cancel, None).await
}
async fn run_trace(
    native: bool,
    source: &str,
    host: Arc<Host>,
    limits: ScriptLimits,
    cancel: Cancellation,
    trace: Option<runtime::Trace>,
) -> ScriptOutcome {
    let inputs = &[("request_id", Type::I64)];
    if native {
        runtime::execute_script(
            Rc::new(CodeUnit::compile_with_declarations(source, inputs, &declarations()).unwrap()),
            host,
            &[7],
            limits,
            cancel,
            trace,
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
            trace,
        )
        .await
    }
}
#[tokio::test]
async fn cv04_original_and_ordered_lazy_fields() {
    for native in [false, true] {
        for (data, expected) in [
            (r#"{"results":[]}"#, "0\n"),
            (r#"{"results":[{"score":81,"id":7}]}"#, "7\n"),
            (r#"{"results":[{"score":80},{"score":82,"id":9}]}"#, "9\n"),
            (r#"{"results":[{"score":1},{"score":2}]}"#, "0\n"),
            (
                r#"{"results":[{"score":81,"id":7},null,{"score":"bad"}]}"#,
                "7\n",
            ),
            (
                r#"{"results":[{"score":0,"score":99},{"score":99,"id":8,"id":9}]}"#,
                "8\n",
            ),
            (
                r#"{"results":[{"sc\u006fre":99,"id":6}]} outer suffix"#,
                "6\n",
            ),
        ] {
            let host = Arc::new(Host::new(data));
            let out = run(
                native,
                CV,
                host.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Ok(()), "native={native} data={data}");
            assert_eq!(out.stdout, expected);
            assert_eq!(*host.calls.lock().unwrap(), vec![101]);
        }
    }
}
#[tokio::test]
async fn general_body_break_skips_remainder_and_cache_resets() {
    let source = "data=tools.search(request_id)\ntotal=0\nfor item in data['results']:\n    if item['score'] > 80:\n        total += item['id'] * 2\n        break\n        total += 1000\n    total += item['score']\nprint(total)\ncheckpoint=tools.checkpoint(request_id)\nfor row in data['results']:\n    total += row['score']\nprint(total)\n";
    for native in [false, true] {
        let mut limits = ScriptLimits::default();
        limits.limits.chunk = 1;
        let out = run(
            native,
            source,
            Arc::new(Host::new(
                r#"{"results":[{"score":2},{"score":90,"id":4},{"score":3}]}"#,
            )),
            limits,
            Cancellation::default(),
        )
        .await;
        assert_eq!(out.status, Ok(()));
        assert_eq!(out.stdout, "10\n105\n");
    }
}
#[tokio::test]
async fn reached_errors_and_prior_output() {
    let source = format!("print(42)\n{CV}");
    for native in [false, true] {
        for data in [
            r#"{}"#,
            r#"{"results":null}"#,
            r#"{"results":{}}"#,
            r#"{"results":[null]}"#,
            r#"{"results":[{}]}"#,
            r#"{"results":[{"score":null}]}"#,
            r#"{"results":[{"score":true}]}"#,
            r#"{"results":[{"score":81}]}"#,
            r#"{"results":[{"score":9223372036854775808}]}"#,
            r#"{"results":[{"score":81,"id":7},garbage]}"#,
            r#"{"results":[{"score":1},]}"#,
        ] {
            let out = run(
                native,
                &source,
                Arc::new(Host::new(data)),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert!(
                matches!(out.status, Err(TaskError::Json(_))),
                "native={native} data={data} {:?}",
                out.status
            );
            assert_eq!(out.stdout, "42\n");
        }
    }
}
#[tokio::test]
async fn exact_visited_fuel_and_chunk_edges() {
    for native in [false, true] {
        for (data, rows, ok) in [
            (r#"{"results":[]}"#, 0, true),
            (r#"{"results":[{"score":81,"id":7},null]}"#, 1, true),
            (r#"{"results":[{"score":0},{"score":81,"id":7}]}"#, 1, false),
            (r#"{"results":[{"score":0},{"score":81,"id":7}]}"#, 2, true),
        ] {
            for chunk in [1, 2, 4096] {
                let mut limits = ScriptLimits::default();
                limits.limits.rows = rows;
                limits.limits.chunk = chunk;
                let out = run(
                    native,
                    CV,
                    Arc::new(Host::new(data)),
                    limits,
                    Cancellation::default(),
                )
                .await;
                if ok {
                    assert_eq!(out.status, Ok(()));
                } else {
                    assert_eq!(out.status, Err(TaskError::Budget));
                }
            }
        }
    }
}
struct Owned(Arc<[u8]>);
impl AsRef<[u8]> for Owned {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[tokio::test]
async fn owner_across_real_wait_last_use_and_wait_cancellation() {
    for native in [false, true] {
        for (future_use, cancelling) in [(false, false), (true, false), (true, true)] {
            let bytes: Arc<[u8]> = Arc::from(br#"{"results":[{"score":81,"id":7}]}"#.as_slice());
            let weak = Arc::downgrade(&bytes);
            let mut h = Host::new(Bytes::from_owner(Owned(bytes)));
            h.wait = true;
            let host = Arc::new(h);
            let suffix = if future_use {
                "for row in data['results']:\n    print(row['id'])\n"
            } else {
                "print(9)\n"
            };
            let source = format!("{CV}\ncheckpoint=tools.checkpoint(request_id)\n{suffix}");
            let cancel = Cancellation::default();
            let waiter = async {
                host.entered.notified().await;
                assert_eq!(weak.upgrade().is_some(), future_use);
                if cancelling {
                    cancel.cancel();
                }
                host.finish.notify_one();
            };
            let (out, ()) = tokio::join!(
                run(
                    native,
                    &source,
                    host.clone(),
                    ScriptLimits::default(),
                    cancel.clone()
                ),
                waiter
            );
            assert_eq!(
                out.status,
                if cancelling {
                    Err(TaskError::Cancelled)
                } else {
                    Ok(())
                }
            );
            assert_eq!(
                out.stdout,
                if cancelling {
                    "7\n"
                } else if future_use {
                    "7\n7\n"
                } else {
                    "7\n9\n"
                }
            );
            assert!(weak.upgrade().is_none());
        }
    }
}

#[tokio::test]
async fn first_chunk_cancellation_drops_cursor_and_prevents_later_tool() {
    use std::{
        cell::RefCell,
        future::{Future, poll_fn},
        task::Poll,
    };
    for native in [false, true] {
        let bytes: Arc<[u8]> =
            Arc::from(br#"{"results":[{"score":1},{"score":2},{"score":3}]}"#.as_slice());
        let weak = Arc::downgrade(&bytes);
        let host = Arc::new(Host::new(Bytes::from_owner(Owned(bytes))));
        let source = "print(42)\ndata=tools.search(request_id)\nfor row in data['results']:\n    print(row['score'])\ncheckpoint=tools.checkpoint(request_id)";
        let trace: runtime::Trace = Rc::new(RefCell::new(vec![]));
        let cancel = Cancellation::default();
        let mut limits = ScriptLimits::default();
        limits.limits.chunk = 1;
        let future = run_trace(
            native,
            source,
            host.clone(),
            limits,
            cancel.clone(),
            Some(trace.clone()),
        );
        tokio::pin!(future);
        let mut observed = false;
        let out = poll_fn(|cx| {
            let result = future.as_mut().poll(cx);
            if !observed && trace.borrow().iter().any(|e| e.phase == "yield") {
                assert!(matches!(result, Poll::Pending));
                assert!(weak.upgrade().is_some());
                observed = true;
                cancel.cancel();
                cx.waker().wake_by_ref();
            }
            result
        })
        .await;
        assert!(observed);
        assert_eq!(out.status, Err(TaskError::Cancelled));
        assert_eq!(out.stdout, "42\n1\n");
        assert_eq!(*host.calls.lock().unwrap(), vec![101]);
        assert!(weak.upgrade().is_none());
    }
}

#[tokio::test]
async fn bounds_overflow_unicode_and_effect_order() {
    for native in [false, true] {
        for (count, ok) in [(255, true), (256, true), (257, false)] {
            let data = format!(
                "{{\"results\":[{}]}}",
                vec!["{\"score\":1}"; count].join(",")
            );
            let source = "data=tools.search(request_id)\ntotal=0\nfor row in data['results']:\n    total+=row['score']\nprint(total)\ncheckpoint=tools.checkpoint(request_id)";
            let host = Arc::new(Host::new(data));
            let mut limits = ScriptLimits::default();
            limits.limits.chunk = 7;
            let out = run(
                native,
                source,
                host.clone(),
                limits,
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, if ok { Ok(()) } else { Err(TaskError::Budget) });
            assert_eq!(host.calls.lock().unwrap().len(), if ok { 2 } else { 1 });
        }
        for (array_len, ok) in [(65535, true), (65536, true), (65537, false)] {
            let data = format!("{{\"results\":[{}]}}", " ".repeat(array_len - 2));
            let out = run(
                native,
                CV,
                Arc::new(Host::new(data)),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, if ok { Ok(()) } else { Err(TaskError::Budget) });
        }
        for (expr, error) in [
            (
                "row['score']+9223372036854775807*2",
                TaskError::Json("expected signed-i64 integer"),
            ),
            ("9223372036854775807*2+row['score']", TaskError::Overflow),
        ] {
            let source = format!(
                "print(42)\ndata=tools.search(request_id)\nfor row in data['results']:\n    n={expr}\ncheckpoint=tools.checkpoint(request_id)"
            );
            let host = Arc::new(Host::new(r#"{"results":[{"score":null}]}"#));
            let out = run(
                native,
                &source,
                host.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Err(error));
            assert_eq!(out.stdout, "42\n");
            assert_eq!(*host.calls.lock().unwrap(), vec![101]);
        }
        let source = "data=tools.search(request_id)\nfor row in data['results']:\n    if True or row['score']>0:\n        print(1)\n        break";
        let out = run(
            native,
            source,
            Arc::new(Host::new(r#"{"results":[{"ignored":"\ud800"}]}"#)),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(out.status, Ok(()));
        assert_eq!(out.stdout, "1\n");
        let invalid = b"{\"results\":[{\"score\":81,\"id\":7,\"ignored\":\"\xff\"}]}".to_vec();
        let out = run(
            native,
            CV,
            Arc::new(Host::new(invalid)),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert!(matches!(out.status, Err(TaskError::Json(_))));
        assert_eq!(out.stdout, "");
    }
}

#[tokio::test]
async fn metadata_stdout_and_cumulative_fuel_bounds() {
    let data = r#"{"results":[{"score":81,"id":7}]}"#;
    for native in [false, true] {
        for (cap, ok) in [(data.len() + 55, false), (data.len() + 56, true)] {
            let mut limits = ScriptLimits::default();
            limits.limits.buffer_bytes = cap;
            let out = run(
                native,
                CV,
                Arc::new(Host::new(data)),
                limits,
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, if ok { Ok(()) } else { Err(TaskError::Budget) });
        }
        let source = "data=tools.search(request_id)\nfor row in data['results']:\n    print(row['id'])\n    if row['id']>0:\n        break\ncheckpoint=tools.checkpoint(request_id)\nfor row in data['results']:\n    print(row['id'])";
        let mut limits = ScriptLimits::default();
        limits.limits.rows = 1;
        let out = run(
            native,
            source,
            Arc::new(Host::new(data)),
            limits,
            Cancellation::default(),
        )
        .await;
        assert_eq!(out.status, Err(TaskError::Budget));
        assert_eq!(out.stdout, "7\n");
        let limits = ScriptLimits {
            stdout_bytes: 1,
            ..ScriptLimits::default()
        };
        let out = run(
            native,
            CV,
            Arc::new(Host::new(data)),
            limits,
            Cancellation::default(),
        )
        .await;
        assert_eq!(out.status, Err(TaskError::Budget));
        assert!(out.stdout.is_empty());
    }
}

#[tokio::test]
async fn shared_prepared_units_do_not_share_cursor_or_scalar_state() {
    let inputs = &[("request_id", Type::I64)];
    let a = Rc::new(CodeUnit::compile_with_declarations(CV, inputs, &declarations()).unwrap());
    let b =
        Rc::new(InterpreterUnit::prepare_with_declarations(CV, inputs, &declarations()).unwrap());
    for score in [81, 0, 82] {
        let data = format!("{{\"results\":[{{\"score\":{score},\"id\":{score}}}]}}");
        let fa = runtime::execute_script(
            a.clone(),
            Arc::new(Host::new(data.clone())),
            &[7],
            ScriptLimits::default(),
            Cancellation::default(),
            None,
        );
        let fb = interpreter::execute_script(
            b.clone(),
            Arc::new(Host::new(data)),
            &[7],
            ScriptLimits::default(),
            Cancellation::default(),
            None,
        );
        println!(
            "DG array futures native={} tape={}",
            std::mem::size_of_val(&fa),
            std::mem::size_of_val(&fb)
        );
        let (oa, ob) = tokio::join!(fa, fb);
        assert_eq!(oa.status, Ok(()));
        assert_eq!(ob.status, Ok(()));
        let expected = format!("{}\n", if score > 80 { score } else { 0 });
        assert_eq!(oa.stdout, expected);
        assert_eq!(ob.stdout, expected);
    }
}

#[tokio::test]
async fn quote_and_array_share_fuel_and_aggregate_memory() {
    let data = r#"{"results":[{"score":81,"id":7}]}"#;
    let source = "rows=tools.load_quotes(request_id)\ndata=tools.search(request_id)\ntotal=0\nfor item in data['results']:\n    total+=item['id']\n    break\nfor quote in rows:\n    total+=quote.unit_price_cents\nprint(total)";
    for native in [false, true] {
        for (fuel, extra, ok) in [(2, 72, true), (2, 71, false), (1, 72, false)] {
            let mut limits = ScriptLimits::default();
            limits.limits.rows = fuel;
            limits.limits.buffer_bytes = data.len() + extra;
            let out = run(
                native,
                source,
                Arc::new(Host::new(data)),
                limits,
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, if ok { Ok(()) } else { Err(TaskError::Budget) });
            assert_eq!(out.stdout, if ok { "9\n" } else { "" });
        }
        let alias = "data=tools.search(request_id)\na=data['results']\nselected=0\nfor row in a:\n    if row['score']>80:\n        selected=row['id']\n        break\n    else:\n        break\n    selected=999\nprint(selected)";
        let out = run(
            native,
            alias,
            Arc::new(Host::new(data)),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(out.status, Ok(()));
        assert_eq!(out.stdout, "7\n");
    }
}

#[test]
fn array_schema_validation_and_static_field_binding() {
    for schema in [
        ObjectArraySchema {
            path: vec![],
            i64_fields: vec!["id".into()],
        },
        ObjectArraySchema {
            path: vec!["x".into()],
            i64_fields: vec![],
        },
        ObjectArraySchema {
            path: vec!["x".into()],
            i64_fields: vec!["id".into(), "id".into()],
        },
        ObjectArraySchema {
            path: vec!["x".into()],
            i64_fields: vec!["x".repeat(257)],
        },
    ] {
        let mut tools = declarations();
        tools[0].result = ResultSchema::JsonObjectArray(Box::new(schema));
        let e = frontend::parse_with_declarations(CV, &[("request_id", Type::I64)], &tools)
            .unwrap_err();
        assert_eq!(e.kind, AdmissionKind::Schema);
    }
    let e = frontend::parse_with_declarations(
        "data=tools.search(request_id)\nfor row in data['results']:\n    print(row['undeclared'])",
        &[("request_id", Type::I64)],
        &declarations(),
    )
    .unwrap_err();
    assert_eq!(e.kind, AdmissionKind::Schema);
}

#[test]
fn unsupported_source_is_whole_source_rejection() {
    for body in [
        "break",
        "for item in data['results']:\n    continue",
        "for item in data['results']:\n    for other in data['results']:\n        print(1)",
        "for item in data['results']:\n    print(item)",
        "for item in data['results']:\n    n=tools.checkpoint(request_id)",
        "print(len(data['results']))",
        "print(data['results'][0]['id'])",
        "for item in data['results']:\n    n=item['id']\nprint(n)",
        "for item in data['results']:\n    break\nprint(item['id'])",
    ] {
        let source = format!("data=tools.search(request_id)\n{body}\n");
        let error = frontend::parse_with_declarations(
            &source,
            &[("request_id", Type::I64)],
            &declarations(),
        )
        .unwrap_err();
        assert!(
            matches!(error.kind, AdmissionKind::Unsupported | AdmissionKind::Type),
            "{source}: {error:?}"
        );
    }
}

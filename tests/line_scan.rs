#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
use bytes::Bytes;
use minipy_native::{
    buffer::QuoteBuffer,
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    ir::AdmissionKind,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits, ScriptOutcome},
    tool::{ArgumentSchema, JsonLeaf, Reply, ResultSchema, ToolArguments, ToolDeclaration},
};
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};
fn declarations() -> Vec<ToolDeclaration> {
    vec![
        ToolDeclaration {
            id: 201,
            name: "log".into(),
            arguments: vec![],
            result: ResultSchema::JsonTyped(vec![(vec!["content".into()], JsonLeaf::String)]),
        },
        ToolDeclaration {
            id: 202,
            name: "checkpoint".into(),
            arguments: vec![ArgumentSchema::I64("count".into())],
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
    async fn call_bound_tool(&self, id: u32, _: ToolArguments) -> Result<Reply, TaskError> {
        self.calls.lock().unwrap().push(id);
        if id == 201 {
            Ok(Reply::Json(self.data.lock().unwrap().take().unwrap()))
        } else {
            if self.wait {
                self.entered.notify_one();
                self.finish.notified().await;
            } else {
                tokio::task::yield_now().await;
            }
            Ok(Reply::I64(9))
        }
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
    if native {
        runtime::execute_script(
            Rc::new(CodeUnit::compile_with_declarations(source, &[], &declarations()).unwrap()),
            host,
            &[],
            limits,
            cancel,
            trace,
        )
        .await
    } else {
        interpreter::execute_script(
            Rc::new(
                InterpreterUnit::prepare_with_declarations(source, &[], &declarations()).unwrap(),
            ),
            host,
            &[],
            limits,
            cancel,
            trace,
        )
        .await
    }
}
const COUNT: &str = "data=tools.log()\ntext=data.content\ncount=0\nfor line in text.splitlines():\n    if line.startswith('WARN:'):\n        count += 1\nprint(count)";
#[tokio::test]
async fn cpython_splitlines_oracle() {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("../bench/results/dr/splitlines-oracle.json")).unwrap();
    let source = "data=tools.log()\nfor line in data.content.splitlines():\n    print(line)";
    let tape =
        Rc::new(InterpreterUnit::prepare_with_declarations(source, &[], &declarations()).unwrap());
    let code = Rc::new(CodeUnit::compile_with_declarations(source, &[], &declarations()).unwrap());
    let rows = data["python_splitlines"].as_array().unwrap();
    assert_eq!(rows.len(), 181);
    for row in rows {
        let expected: String = row["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| format!("{}\n", l.as_str().unwrap()))
            .collect();
        for escaped in [false, true] {
            let raw = serde_json::json!({"content":row["text"]}).to_string();
            // Force JSON decoding even for ASCII-only source strings.
            let raw = if escaped {
                raw.replacen("\"content\":\"", "\"content\":\"\\u0058", 1)
            } else {
                raw
            };
            let expected = if escaped {
                let text = format!("X{}", row["text"].as_str().unwrap());
                // The injected X changes only the first line (or creates one).
                if text == "X" {
                    "X\n".to_owned()
                } else {
                    format!("X{expected}")
                }
            } else {
                expected.clone()
            };
            let mut limits = ScriptLimits::default();
            limits.limits.chunk = 1;
            for native in [false, true] {
                let host = Arc::new(Host::new(raw.clone()));
                let out = if native {
                    runtime::execute_script(
                        code.clone(),
                        host,
                        &[],
                        limits,
                        Cancellation::default(),
                        None,
                    )
                    .await
                } else {
                    interpreter::execute_script(
                        tape.clone(),
                        host,
                        &[],
                        limits,
                        Cancellation::default(),
                        None,
                    )
                    .await
                };
                assert_eq!(out.status, Ok(()), "{row:?}");
                assert_eq!(out.stdout, expected, "{row:?}");
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
async fn owner_retention_across_wait_and_cancellation() {
    for native in [false, true] {
        for escaped in [false, true] {
            for (future_use, cancelling) in [(false, false), (true, false), (true, true)] {
                let raw = if escaped {
                    br#"{"content":"WARN:x\nlast"}"#.as_slice()
                } else {
                    br#"{"content":"WARN:x"}"#.as_slice()
                };
                let bytes: Arc<[u8]> = Arc::from(raw);
                let weak = Arc::downgrade(&bytes);
                let mut h = Host::new(Bytes::from_owner(Owned(bytes)));
                h.wait = true;
                let host = Arc::new(h);
                let source = format!(
                    "{COUNT}\nx=tools.checkpoint(count)\n{}",
                    if future_use {
                        "print(text)"
                    } else {
                        "print(x)"
                    }
                );
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
                        "1\n"
                    } else if !future_use {
                        "1\n9\n"
                    } else if escaped {
                        "1\nWARN:x\nlast\n"
                    } else {
                        "1\nWARN:x\n"
                    }
                );
                assert!(weak.upgrade().is_none());
            }
        }
    }
}
#[tokio::test]
async fn chunk_cancellation_preserves_prefix_and_prevents_later_tool() {
    use std::{
        cell::RefCell,
        future::{Future, poll_fn},
        task::Poll,
    };
    for native in [false, true] {
        let bytes: Arc<[u8]> = Arc::from(br#"{"content":"WARN:x\nWARN:y\nWARN:z"}"#.as_slice());
        let weak = Arc::downgrade(&bytes);
        let host = Arc::new(Host::new(Bytes::from_owner(Owned(bytes))));
        let trace: runtime::Trace = Rc::new(RefCell::new(vec![]));
        let cancel = Cancellation::default();
        let mut limits = ScriptLimits::default();
        limits.limits.chunk = 1;
        let future = run_trace(
            native,
            FILTER,
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
        assert_eq!(out.stdout, "WARN:x\n");
        assert_eq!(*host.calls.lock().unwrap(), vec![201]);
        assert!(weak.upgrade().is_none());
    }
}
#[tokio::test]
async fn scan_budget_long_line_break_short_circuit_and_permission() {
    for native in [false, true] {
        // Raw-token limit includes its quotes. A single decoded line need not
        // fit default stdout; counting or break must not copy it to a sink.
        for (len, ok) in [(65534, true), (65535, false)] {
            let source =
                "data=tools.log()\nfor line in data.content.splitlines():\n    print(1)\n    break";
            let mut limits = ScriptLimits::default();
            limits.limits.rows = 1;
            let out = run(
                native,
                source,
                Arc::new(Host::new(
                    serde_json::json!({"content":"x".repeat(len)}).to_string(),
                )),
                limits,
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, if ok { Ok(()) } else { Err(TaskError::Budget) });
            assert_eq!(out.stdout, if ok { "1\n" } else { "" });
        }
        let source = "data=tools.log()\nfor line in data.content.splitlines():\n    if True or 'expensive' in line:\n        print(line)\n        break\nx=tools.checkpoint(1)";
        // JSON escape decoding costs raw token bytes, first-line scan 3
        // (é + LF), and print 2. Break never scans the remaining x.
        let raw = r#"{"content":"é\nx"}"#;
        let token = serde_json::to_string("é\nx").unwrap().len();
        for (budget, ok) in [(token + 5, true), (token + 4, false)] {
            let mut limits = ScriptLimits::default();
            limits.string_scan_bytes = budget;
            limits.limits.rows = 1;
            let host = Arc::new(Host::new(raw));
            let out = run(
                native,
                source,
                host.clone(),
                limits,
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, if ok { Ok(()) } else { Err(TaskError::Budget) });
            assert_eq!(out.stdout, if ok { "é\n" } else { "" });
            assert_eq!(host.calls.lock().unwrap().len(), if ok { 2 } else { 1 });
        }
        let mut limits = ScriptLimits::default();
        limits.allow_print = false;
        for (raw, error) in [
            (r#"{"content":"x"}"#, TaskError::Permission),
            (r#"{"content":null}"#, TaskError::Json("expected string")),
        ] {
            let out = run(
                native,
                "data=tools.log()\nfor line in data.content.splitlines():\n    print(line)",
                Arc::new(Host::new(raw)),
                limits,
                Cancellation::default(),
            )
            .await;
            if matches!(error, TaskError::Json(_)) {
                assert!(matches!(out.status, Err(TaskError::Json(_))));
            } else {
                assert_eq!(out.status, Err(error));
            }
            assert_eq!(out.stdout, "");
        }
        let source = "print(42)\ndata=tools.log()\nn=9223372036854775807\nfor line in data.content.splitlines():\n    n+=1\nx=tools.checkpoint(1)";
        let host = Arc::new(Host::new(r#"{"content":"x"}"#));
        let out = run(
            native,
            source,
            host.clone(),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(out.status, Err(TaskError::Overflow));
        assert_eq!(out.stdout, "42\n");
        assert_eq!(*host.calls.lock().unwrap(), vec![201]);
    }
}
const FILTER: &str = "data=tools.log()\ncount=0\nfor line in data['content'].splitlines():\n    if 'WARN:' in line and not line.startswith('skip'):\n        print(line)\n        count += 1\nprint(count)\nresult=tools.checkpoint(count)\nprint(result)";
#[tokio::test]
async fn complete_count_and_filter_both_backends() {
    for native in [false, true] {
        for (text, count, filtered) in [
            ("", "0\n", "0\n9\n"),
            (
                "WARN: café\r\nno\rWARN: 😀\u{85}skip WARN:\u{2028}",
                "2\n",
                "WARN: café\nWARN: 😀\n2\n9\n",
            ),
            ("\nWARN:\n\n", "1\n", "WARN:\n1\n9\n"),
        ] {
            for (source, expected) in [(COUNT, count), (FILTER, filtered)] {
                let mut limits = ScriptLimits::default();
                limits.limits.chunk = 1;
                let host = Arc::new(Host::new(serde_json::json!({"content":text}).to_string()));
                let out = run(
                    native,
                    source,
                    host.clone(),
                    limits,
                    Cancellation::default(),
                )
                .await;
                assert_eq!(out.status, Ok(()));
                assert_eq!(out.stdout, expected);
                assert_eq!(
                    *host.calls.lock().unwrap(),
                    if source == COUNT {
                        vec![201]
                    } else {
                        vec![201, 202]
                    }
                );
            }
        }
    }
}
#[test]
fn reject_escaping_and_dynamic_lines_whole_source() {
    for tail in [
        "for line in text.splitlines(True):\n    print(1)",
        "for line in text.splitlines(keepends=False):\n    print(1)",
        "lines=text.splitlines()",
        "for line in text.splitlines():\n    other=line",
        "for line in text.splitlines():\n    line=7",
        "for line in text.splitlines():\n    tools.checkpoint(1)",
        "for line in text.splitlines():\n    for x in text.splitlines():\n        print(1)",
        "for line in text.splitlines():\n    continue",
        "for line in text.splitlines():\n    print(line)\nelse:\n    print(1)",
        "for line in text.splitlines():\n    print(line)\nprint(line)",
        "for line in text.splitlines():\n    print(line == 'x')",
        "for line in text.splitlines():\n    print(line.quantity)",
        "for text in text.splitlines():\n    print(1)",
        "for line in text.splitlines():\n    fresh=1\nprint(fresh)",
    ] {
        let source = format!("print(42)\ndata=tools.log()\ntext=data.content\n{tail}");
        let a = CodeUnit::compile_with_declarations(&source, &[], &declarations())
            .err()
            .expect(tail);
        let b = InterpreterUnit::prepare_with_declarations(&source, &[], &declarations())
            .err()
            .expect(tail);
        assert_eq!(a, b, "{tail}");
        assert_eq!(a.kind, AdmissionKind::Unsupported, "{tail}");
    }
}
#[tokio::test]
async fn more_than_256_lines_and_exact_fuel() {
    let raw = serde_json::json!({"content":"WARN:x\n".repeat(300)}).to_string();
    for native in [false, true] {
        for (fuel, ok) in [(300, true), (299, false)] {
            let mut limits = ScriptLimits::default();
            limits.limits.rows = fuel;
            limits.limits.chunk = 7;
            let out = run(
                native,
                COUNT,
                Arc::new(Host::new(raw.clone())),
                limits,
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, if ok { Ok(()) } else { Err(TaskError::Budget) });
            assert_eq!(out.stdout, if ok { "300\n" } else { "" });
        }
    }
}
#[tokio::test]
async fn entry_validation_and_atomic_output_errors() {
    let source = "print(42)\ndata=tools.log()\nfor line in data.content.splitlines():\n    print(line)\n    break\nx=tools.checkpoint(1)";
    for native in [false, true] {
        for raw in [
            b"{\"content\":\"ok\\n\xff\"}".as_slice(),
            br#"{"content":"ok\n\ud800"}"#,
            br#"{"content":null}"#,
            br#"{"content":3}"#,
            b"{}",
        ] {
            let host = Arc::new(Host::new(Bytes::copy_from_slice(raw)));
            let out = run(
                native,
                source,
                host.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert!(matches!(out.status, Err(TaskError::Json(_))));
            assert_eq!(out.stdout, "42\n");
            assert_eq!(*host.calls.lock().unwrap(), vec![201]);
        }
        let mut limits = ScriptLimits::default();
        limits.stdout_bytes = 6;
        let host = Arc::new(Host::new(r#"{"content":"long line\nlast"}"#));
        let out = run(
            native,
            source,
            host.clone(),
            limits,
            Cancellation::default(),
        )
        .await;
        assert_eq!(out.status, Err(TaskError::Budget));
        assert_eq!(out.stdout, "42\n");
        assert_eq!(*host.calls.lock().unwrap(), vec![201]);
    }
}

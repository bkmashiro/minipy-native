#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
use bytes::Bytes;
use minipy_native::{
    buffer::QuoteBuffer,
    codegen::CodeUnit,
    error::TaskError,
    frontend,
    interpreter::{self, InterpreterUnit},
    ir::AdmissionKind,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits, ScriptOutcome},
    tool::{
        ArgumentSchema, JsonLeaf, Reply, ResultSchema, ToolArguments, ToolDeclaration, ToolSchema,
    },
};
use std::{
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::Notify;

struct Host {
    calls: Mutex<Vec<(u32, String)>>,
    entered: Notify,
    finish: Notify,
    pending: AtomicUsize,
    delayed: bool,
}
impl Host {
    fn new(delayed: bool) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            entered: Notify::new(),
            finish: Notify::new(),
            pending: AtomicUsize::new(0),
            delayed,
        }
    }
}
impl Provider for Host {
    fn allows_tool(&self, id: u32) -> bool {
        id != 12
    }
    async fn call_bound_tool(&self, id: u32, args: ToolArguments) -> Result<Reply, TaskError> {
        let ToolArguments::String(value) = args else {
            return Err(TaskError::Input);
        };
        self.pending.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        if self.delayed {
            self.finish.notified().await;
        }
        let path = value.as_str();
        self.calls.lock().unwrap().push((id, path.to_owned()));
        self.pending.fetch_sub(1, Ordering::SeqCst);
        match (id, path) {
            (10, "report.txt") => Ok(Reply::Json(Bytes::from_static(
                "{\"content\":\"hello 🌍\"}".as_bytes(),
            ))),
            (10, "report.42") => Ok(Reply::Json(Bytes::from_static(b"{\"content\":\"hello\"}"))),
            (10, "escaped") => Ok(Reply::Json(Bytes::from_static(
                br#"{"content":"report\u002etxt"}"#,
            ))),
            (10, "long") => Ok(Reply::Json(Bytes::from(format!(
                "{{\"content\":\"{}\"}}",
                "x".repeat(40_000)
            )))),
            (11, "report.txt") => Ok(Reply::Json(Bytes::from_static(br#"{"content":"second"}"#))),
            (11, "hello 🌍") => Ok(Reply::Json(Bytes::from_static(br#"{"content":"second"}"#))),
            (10, "missing") => Err(TaskError::Provider("missing".into())),
            (10, "bad") => Ok(Reply::Json(Bytes::from_static(br#"{"content":null}"#))),
            _ => Err(TaskError::Permission),
        }
    }
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        Err(TaskError::Permission)
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
}
fn tools() -> Vec<ToolDeclaration> {
    [(10, "read_file"), (11, "read_other"), (12, "denied")]
        .into_iter()
        .map(|(id, name)| ToolDeclaration {
            id,
            name: name.into(),
            arguments: vec![ArgumentSchema::String("path".into())],
            result: ResultSchema::JsonTyped(vec![(vec!["content".into()], JsonLeaf::String)]),
        })
        .collect()
}
async fn run<P: Provider>(
    native: bool,
    source: &str,
    host: Arc<P>,
    limits: ScriptLimits,
    cancel: Cancellation,
) -> ScriptOutcome {
    if native {
        runtime::execute_script(
            Rc::new(CodeUnit::compile_with_declarations(source, &[], &tools()).unwrap()),
            host,
            &[],
            limits,
            cancel,
            None,
        )
        .await
    } else {
        interpreter::execute_script(
            Rc::new(InterpreterUnit::prepare_with_declarations(source, &[], &tools()).unwrap()),
            host,
            &[],
            limits,
            cancel,
            None,
        )
        .await
    }
}
#[tokio::test]
async fn literal_string_tools_are_not_special_cased_and_errors_preserve_output() {
    for native in [false, true] {
        for (name, expected) in [("read_file", "hello 🌍\n"), ("read_other", "second\n")] {
            let host = Arc::new(Host::new(false));
            let source = format!("data = tools.{name}(\"report.txt\")\nprint(data[\"content\"])");
            let out = run(
                native,
                &source,
                host.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Ok(()));
            assert_eq!(out.stdout, expected);
            assert_eq!(host.calls.lock().unwrap().len(), 1);
        }
        for (source, expected) in [
            (
                "print(1)\ndata = tools.denied(\"report.txt\")\nprint(data[\"content\"])",
                TaskError::Permission,
            ),
            (
                "print(1)\ndata = tools.read_file(\"missing\")\nprint(data[\"content\"])",
                TaskError::Provider("missing".into()),
            ),
            (
                "print(1)\ndata = tools.read_file(\"bad\")\nprint(data[\"content\"])",
                TaskError::Json("expected UTF-8 string"),
            ),
        ] {
            let host = Arc::new(Host::new(false));
            let out = run(
                native,
                source,
                host,
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Err(expected));
            assert_eq!(out.stdout, "1\n");
        }
        let host = Arc::new(Host::new(false));
        let out = run(
            native,
            "print(1)\ndata = tools.read_file(\"report.txt\")\nprint(data[\"content\"])",
            host,
            ScriptLimits {
                stdout_bytes: 3,
                ..ScriptLimits::default()
            },
            Cancellation::default(),
        )
        .await;
        assert_eq!(out.status, Err(TaskError::Budget));
        assert_eq!(out.stdout, "1\n");
    }
}
#[tokio::test]
async fn json_alias_escaped_utf8_and_detached_owner_survive_cancellation() {
    for native in [false, true] {
        for escaped in [false, true] {
            let schema = vec![
                ToolDeclaration {
                    id: 13,
                    name: "source".into(),
                    arguments: vec![ArgumentSchema::String("path".into())],
                    result: ResultSchema::JsonTyped(vec![(
                        vec!["content".into()],
                        JsonLeaf::String,
                    )]),
                },
                tools().remove(0),
            ];
            // The first reply is supplied by an inline fixture below, testing the
            // already decoded string path rather than manufacturing a dynamic value.
            struct Chain {
                gate: Arc<Host>,
                owner: Mutex<Option<Bytes>>,
            }
            impl Provider for Chain {
                fn allows_tool(&self, _: u32) -> bool {
                    true
                }
                async fn call_bound_tool(
                    &self,
                    id: u32,
                    args: ToolArguments,
                ) -> Result<Reply, TaskError> {
                    if id == 13 {
                        let bytes = self.owner.lock().unwrap().take().unwrap();
                        Ok(Reply::Json(bytes))
                    } else {
                        let ToolArguments::String(arg) = args else {
                            panic!("wrong binding")
                        };
                        self.gate.pending.fetch_add(1, Ordering::SeqCst);
                        self.gate.entered.notify_one();
                        self.gate.finish.notified().await;
                        assert_eq!(arg.as_str(), "report.txt");
                        self.gate.pending.fetch_sub(1, Ordering::SeqCst);
                        Ok(Reply::Json(Bytes::from_static(br#"{"content":"done"}"#)))
                    }
                }
                async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
                    Err(TaskError::Permission)
                }
                async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
                    Err(TaskError::Permission)
                }
            }
            struct Owned(Arc<[u8]>);
            impl AsRef<[u8]> for Owned {
                fn as_ref(&self) -> &[u8] {
                    &self.0
                }
            }
            let backing: Arc<[u8]> = Arc::from(if escaped {
                br#"{"content":"report\u002etxt"}"#.as_slice()
            } else {
                br#"{"content":"report.txt"}"#.as_slice()
            });
            let weak = Arc::downgrade(&backing);
            let bytes = Bytes::from_owner(Owned(backing));
            let gate = Arc::new(Host::new(true));
            let host = Arc::new(Chain {
                gate: gate.clone(),
                owner: Mutex::new(Some(bytes)),
            });
            let source = "s = tools.source(path=\"unused\")\nname = s[\"content\"]\ndata = tools.read_file(name)\nprint(data[\"content\"])";
            let cancel = Cancellation::default();
            let task = async {
                if native {
                    runtime::execute_script(
                        Rc::new(CodeUnit::compile_with_declarations(source, &[], &schema).unwrap()),
                        host,
                        &[],
                        ScriptLimits::default(),
                        cancel.clone(),
                        None,
                    )
                    .await
                } else {
                    interpreter::execute_script(
                        Rc::new(
                            InterpreterUnit::prepare_with_declarations(source, &[], &schema)
                                .unwrap(),
                        ),
                        host,
                        &[],
                        ScriptLimits::default(),
                        cancel.clone(),
                        None,
                    )
                    .await
                }
            };
            tokio::pin!(task);
            tokio::select! { _ = gate.entered.notified() => {}, outcome = &mut task => panic!("unexpected: {:?}", outcome.status) }
            cancel.cancel();
            assert_eq!(task.await.status, Err(TaskError::Cancelled));
            assert_eq!(gate.pending.load(Ordering::SeqCst), 1);
            // Decoded strings own a separate allocation; plain strings borrow the
            // validated reply, held by the detached operation after cancellation.
            assert_eq!(weak.upgrade().is_some(), !escaped);
            gate.finish.notify_one();
            for _ in 0..10 {
                if gate.pending.load(Ordering::SeqCst) == 0 {
                    break;
                }
                tokio::task::yield_now().await;
            }
            assert_eq!(gate.pending.load(Ordering::SeqCst), 0);
            assert!(weak.upgrade().is_none());
            let gate = Arc::new(Host::new(false));
            let schema = vec![
                ToolDeclaration {
                    id: 13,
                    name: "source".into(),
                    arguments: vec![ArgumentSchema::String("path".into())],
                    result: ResultSchema::JsonTyped(vec![(
                        vec!["content".into()],
                        JsonLeaf::String,
                    )]),
                },
                tools().remove(0),
            ];
            let host = Arc::new(Chain {
                gate: gate.clone(),
                owner: Mutex::new(Some(Bytes::from_static(b"{\"content\":\"\xff\"}"))),
            });
            let source =
                "s = tools.source(\"unused\")\nname = s[\"content\"]\ndata = tools.read_file(name)";
            let outcome = if native {
                runtime::execute_script(
                    Rc::new(CodeUnit::compile_with_declarations(source, &[], &schema).unwrap()),
                    host,
                    &[],
                    ScriptLimits::default(),
                    Cancellation::default(),
                    None,
                )
                .await
            } else {
                interpreter::execute_script(
                    Rc::new(
                        InterpreterUnit::prepare_with_declarations(source, &[], &schema).unwrap(),
                    ),
                    host,
                    &[],
                    ScriptLimits::default(),
                    Cancellation::default(),
                    None,
                )
                .await
            };
            assert!(matches!(outcome.status, Err(TaskError::Json(_))));
            assert_eq!(gate.pending.load(Ordering::SeqCst), 0);
        }
    }
}
#[tokio::test]
async fn sink_join_print_and_tool_preserve_order_and_bounds() {
    for native in [false, true] {
        for (source, expected, calls) in [
            ("print(\"é\" + \"🌍\")", "é🌍\n", 0),
            (
                "a = -9223372036854775807 - 1\nb = 9223372036854775807\nprint(f\"{a}{b}{0}{True}{False}\")",
                "-922337203685477580892233720368547758070TrueFalse\n",
                0,
            ),
            (
                "data = tools.read_file(\"report\" + \".txt\")\nname = data[\"content\"]\nprint(\"[\" + name + \"/\" + name + \"]\")",
                "[hello 🌍/hello 🌍]\n",
                1,
            ),
            (
                "data = tools.read_file(\"report.txt\")\nname = data[\"content\"]\nnext = tools.read_other(name)\nprint(next[\"content\"])",
                "second\n",
                2,
            ),
            (
                "n = 42\nb = True\nprint(f\"value={n}, ok={b}, literal={False}\")\nnext = tools.read_file(f\"report.{n}\")\nprint(next[\"content\"])",
                "value=42, ok=True, literal=False\nhello\n",
                1,
            ),
            (
                "data = tools.read_file(\"escaped\")\nname = data[\"content\"]\nprint(f\"path={name}\")\nnext = tools.read_other(name)\nprint(next[\"content\"])",
                "path=report.txt\nsecond\n",
                2,
            ),
            (
                "n = 42\ndata = tools.read_file(\"report.txt\")\nname = data[\"content\"]\nnext = tools.read_other(\"report.txt\")\nprint(f\"{n}:{name}/{next['content']}\")",
                "42:hello 🌍/second\n",
                2,
            ),
            (
                "data = tools.read_file(\"report.txt\")\nname = data[\"content\"]\nignored = tools.read_other(\"report.txt\")\nnext = tools.read_other(name)\nprint(next[\"content\"])",
                "second\n",
                3,
            ),
        ] {
            let host = Arc::new(Host::new(false));
            let outcome = run(
                native,
                source,
                host.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(outcome.status, Ok(()), "{source}");
            assert_eq!(outcome.stdout, expected);
            assert_eq!(host.calls.lock().unwrap().len(), calls);
        }
        let host = Arc::new(Host::new(false));
        let outcome = run(
            native,
            "print(1)\nprint(\"é\" + \"🌍\")",
            host,
            ScriptLimits {
                stdout_bytes: 6,
                ..ScriptLimits::default()
            },
            Cancellation::default(),
        )
        .await;
        assert_eq!(outcome.status, Err(TaskError::Budget));
        assert_eq!(outcome.stdout, "1\n");
        let host = Arc::new(Host::new(false));
        let source = "print(1)\nlong = tools.read_file(\"long\")\ntext = long[\"content\"]\nnext = tools.read_other(text + text)";
        let outcome = run(
            native,
            source,
            host.clone(),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(outcome.status, Err(TaskError::Budget));
        assert_eq!(outcome.stdout, "1\n");
        assert_eq!(host.calls.lock().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn joined_argument_remains_owned_after_cancellation() {
    struct Chain {
        gate: Arc<Host>,
    }
    impl Provider for Chain {
        fn allows_tool(&self, _: u32) -> bool {
            true
        }
        async fn call_bound_tool(&self, id: u32, args: ToolArguments) -> Result<Reply, TaskError> {
            if id == 10 {
                return Ok(Reply::Json(Bytes::from_static(
                    "{\"content\":\"hello 🌍\"}".as_bytes(),
                )));
            }
            let ToolArguments::String(arg) = args else {
                panic!("wrong binding")
            };
            self.gate.pending.fetch_add(1, Ordering::SeqCst);
            self.gate.entered.notify_one();
            self.gate.finish.notified().await;
            assert_eq!(arg.as_str(), "hello 🌍");
            self.gate.pending.fetch_sub(1, Ordering::SeqCst);
            Ok(Reply::Json(Bytes::from_static(br#"{"content":"second"}"#)))
        }
        async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
            Err(TaskError::Permission)
        }
        async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
            Err(TaskError::Permission)
        }
    }
    for native in [false, true] {
        let gate = Arc::new(Host::new(true));
        let host = Arc::new(Chain { gate: gate.clone() });
        let source = "data = tools.read_file(\"report.txt\")\nname = data[\"content\"]\nnext = tools.read_other(name + \"\")";
        let cancel = Cancellation::default();
        let task = run(
            native,
            source,
            host,
            ScriptLimits::default(),
            cancel.clone(),
        );
        tokio::pin!(task);
        tokio::select! { _ = gate.entered.notified() => {}, outcome = &mut task => panic!("unexpected: {:?}", outcome.status) }
        cancel.cancel();
        assert_eq!(task.await.status, Err(TaskError::Cancelled));
        assert_eq!(gate.pending.load(Ordering::SeqCst), 1);
        gate.finish.notify_one();
        for _ in 0..10 {
            if gate.pending.load(Ordering::SeqCst) == 0 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(gate.pending.load(Ordering::SeqCst), 0);
    }
}
#[test]
fn admission_location_and_legacy_numeric_api() {
    let schema = tools();
    for source in [
        "data = tools.read_file(name)",
        "data = tools.read_file(2)",
        "data = tools.read_file(path=\"a\", path=\"b\")",
        "data = tools.read_file(**kw)",
        "print(f\"{1:04d}\")",
        "print(f\"{1!r}\")",
        "print(\"x\" + 1)",
        "x = \"a\" + \"b\"",
    ] {
        let err = frontend::parse_with_declarations(source, &[], &schema).unwrap_err();
        assert!(err.location.is_some(), "{source}: {err:?}");
    }
    let bad = [ToolDeclaration {
        id: 10,
        name: "conflict".into(),
        arguments: vec![ArgumentSchema::I64("a".into())],
        result: ResultSchema::I64,
    }];
    assert_eq!(
        frontend::parse_with_declarations("", &[], &[schema[0].clone(), bad[0].clone()])
            .unwrap_err()
            .kind,
        AdmissionKind::Schema
    );
    let old = ToolSchema {
        id: 15,
        name: "numeric".into(),
        arguments: vec!["value".into()],
        result: ResultSchema::I64,
    };
    assert!(
        CodeUnit::compile_with_tools("print(tools.numeric(3))", &[], std::slice::from_ref(&old))
            .is_ok()
    );
    assert!(InterpreterUnit::prepare_with_tools("print(tools.numeric(3))", &[], &[old]).is_ok());
}

/// Resident process, new source each request. Synthetic local fixture only.
#[tokio::test]
#[ignore]
async fn resident_request_cost() {
    struct Perf;
    impl Provider for Perf {
        fn allows_tool(&self, _: u32) -> bool {
            true
        }
        async fn call_tool(&self, _: u32, _: [i64; 2], _: usize) -> Result<Reply, TaskError> {
            Ok(Reply::Json(Bytes::from_static(br#"{"content":"ok"}"#)))
        }
        async fn call_bound_tool(&self, id: u32, args: ToolArguments) -> Result<Reply, TaskError> {
            match args {
                ToolArguments::Numeric(a, n) => self.call_tool(id, a, n).await,
                ToolArguments::String(s) if s.as_str() == "report.txt" => {
                    Ok(Reply::Json(Bytes::from_static(br#"{"content":"ok"}"#)))
                }
                _ => Err(TaskError::Permission),
            }
        }
        async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
            Err(TaskError::Permission)
        }
        async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
            Err(TaskError::Permission)
        }
    }
    let old = [ToolSchema {
        id: 14,
        name: "read_file".into(),
        arguments: vec!["path".into()],
        result: ResultSchema::JsonTyped(vec![(vec!["content".into()], JsonLeaf::String)]),
    }];
    let new = [ToolDeclaration {
        id: 14,
        name: "read_file".into(),
        arguments: vec![ArgumentSchema::String("path".into())],
        result: ResultSchema::JsonTyped(vec![(vec!["content".into()], JsonLeaf::String)]),
    }];
    let host = Arc::new(Perf);
    for native in [false, true] {
        let mut times = [Vec::new(), Vec::new()];
        for n in 0..26 {
            for string in if n % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let arg = if string { "\"report.txt\"" } else { "1" };
                let source = format!(
                    "data = tools.read_file({arg})\nprint(data[\"content\"])\n# request {n}"
                );
                let start = std::time::Instant::now();
                let result = if native {
                    let unit = if string {
                        CodeUnit::compile_with_declarations(&source, &[], &new)
                    } else {
                        CodeUnit::compile_with_tools(&source, &[], &old)
                    }
                    .unwrap();
                    runtime::execute_script(
                        Rc::new(unit),
                        host.clone(),
                        &[],
                        ScriptLimits::default(),
                        Cancellation::default(),
                        None,
                    )
                    .await
                } else {
                    let unit = if string {
                        InterpreterUnit::prepare_with_declarations(&source, &[], &new)
                    } else {
                        InterpreterUnit::prepare_with_tools(&source, &[], &old)
                    }
                    .unwrap();
                    interpreter::execute_script(
                        Rc::new(unit),
                        host.clone(),
                        &[],
                        ScriptLimits::default(),
                        Cancellation::default(),
                        None,
                    )
                    .await
                };
                let elapsed = start.elapsed().as_nanos();
                assert_eq!(result.status, Ok(()));
                assert_eq!(result.stdout, "ok\n");
                println!(
                    "EVIDENCE {{\"backend\":\"{}\",\"arg\":\"{}\",\"request\":{},\"warmup\":{},\"elapsed_ns\":{}}}",
                    if native { "native" } else { "tape" },
                    if string { "string" } else { "numeric" },
                    n,
                    n < 2,
                    elapsed
                );
                if n >= 2 {
                    times[usize::from(string)].push(elapsed);
                }
            }
        }
        for (string, times) in [false, true].into_iter().zip(times.iter_mut()) {
            times.sort_unstable();
            println!(
                "backend={} arg={} n={} median_ns={} p90_ns={}",
                if native { "native" } else { "tape" },
                if string { "string" } else { "numeric" },
                times.len(),
                times[times.len() / 2],
                times[times.len() * 9 / 10]
            );
        }
    }
}
/// Synthetic source-cold cost of the new sink, using a resident Host reply.
#[tokio::test]
#[ignore]
async fn resident_sink_cost() {
    struct Perf;
    impl Provider for Perf {
        fn allows_tool(&self, _: u32) -> bool {
            true
        }
        async fn call_bound_tool(&self, _: u32, args: ToolArguments) -> Result<Reply, TaskError> {
            if matches!(args, ToolArguments::String(s) if s.as_str() == "report.txt") {
                Ok(Reply::Json(Bytes::from_static(br#"{"content":"ok"}"#)))
            } else {
                Err(TaskError::Permission)
            }
        }
        async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
            Err(TaskError::Permission)
        }
        async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
            Err(TaskError::Permission)
        }
    }
    let declarations = [ToolDeclaration {
        id: 14,
        name: "read_file".into(),
        arguments: vec![ArgumentSchema::String("path".into())],
        result: ResultSchema::JsonTyped(vec![(vec!["content".into()], JsonLeaf::String)]),
    }];
    let host = Arc::new(Perf);
    for native in [false, true] {
        for n in 0..26 {
            let source = format!(
                "data = tools.read_file(\"report\" + \".txt\")\ntext = data[\"content\"]\nprint(f\"[{{text}}]\")\n# request {n}"
            );
            let start = std::time::Instant::now();
            let result = if native {
                let unit =
                    CodeUnit::compile_with_declarations(&source, &[], &declarations).unwrap();
                runtime::execute_script(
                    Rc::new(unit),
                    host.clone(),
                    &[],
                    ScriptLimits::default(),
                    Cancellation::default(),
                    None,
                )
                .await
            } else {
                let unit = InterpreterUnit::prepare_with_declarations(&source, &[], &declarations)
                    .unwrap();
                interpreter::execute_script(
                    Rc::new(unit),
                    host.clone(),
                    &[],
                    ScriptLimits::default(),
                    Cancellation::default(),
                    None,
                )
                .await
            };
            let elapsed = start.elapsed().as_nanos();
            assert_eq!(result.status, Ok(()));
            assert_eq!(result.stdout, "[ok]\n");
            println!(
                "EVIDENCE {{\"backend\":\"{}\",\"arg\":\"joined-sink\",\"request\":{},\"warmup\":{},\"elapsed_ns\":{}}}",
                if native { "native" } else { "tape" },
                n,
                n < 2,
                elapsed
            );
        }
    }
}

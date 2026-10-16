#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
use bytes::Bytes;
use minipy_native::{
    buffer::QuoteBuffer,
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    ir::{AdmissionKind, Type},
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

const INPUTS: &[(&str, Type)] = &[("request_id", Type::I64)];
const CASES: [&str; 10] = [
    include_str!("../fixtures/cv/01_status_summary.py"),
    include_str!("../fixtures/cv/02_json_few_fields.py"),
    include_str!("../fixtures/cv/03_path_then_call.py"),
    include_str!("../fixtures/cv/04_first_matching_result.py"),
    include_str!("../fixtures/cv/05_missing_default.py"),
    include_str!("../fixtures/cv/06_multi_call_branch.py"),
    include_str!("../fixtures/cv/07_exact_route.py"),
    include_str!("../fixtures/cv/08_nonempty_message.py"),
    include_str!("../fixtures/cv/09_prefix_route.py"),
    include_str!("../fixtures/cv/10_alias_contains_summary.py"),
];
fn declarations() -> Vec<ToolDeclaration> {
    let leaves = ["status", "filename", "category", "message", "owner"]
        .into_iter()
        .map(|key| (vec![key.into()], JsonLeaf::String))
        .chain([(vec!["count".into()], JsonLeaf::I64)])
        .collect();
    vec![
        ToolDeclaration {
            id: 101,
            name: "fetch_state".into(),
            arguments: vec![ArgumentSchema::I64("key".into())],
            result: ResultSchema::JsonTyped(leaves),
        },
        ToolDeclaration {
            id: 102,
            name: "fetch_stats".into(),
            arguments: vec![ArgumentSchema::I64("key".into())],
            result: ResultSchema::JsonTyped(vec![
                (vec!["limit".into()], JsonLeaf::I64),
                (vec!["used".into()], JsonLeaf::I64),
                (vec!["label".into()], JsonLeaf::String),
            ]),
        },
        ToolDeclaration {
            id: 103,
            name: "read_text".into(),
            arguments: vec![ArgumentSchema::String("path".into())],
            result: ResultSchema::JsonTyped(vec![(vec!["content".into()], JsonLeaf::String)]),
        },
        ToolDeclaration {
            id: 104,
            name: "emit_text".into(),
            arguments: vec![ArgumentSchema::String("text".into())],
            result: ResultSchema::I64,
        },
        ToolDeclaration {
            id: 105,
            name: "publish".into(),
            arguments: vec![
                ArgumentSchema::I64("key".into()),
                ArgumentSchema::I64("total".into()),
            ],
            result: ResultSchema::I64,
        },
    ]
}
#[derive(Debug, PartialEq)]
enum Call {
    Numeric(u32, Vec<i64>),
    Text(u32, String),
}
struct Host {
    data: Mutex<Option<Bytes>>,
    calls: Mutex<Vec<Call>>,
    gate: Option<u32>,
    entered: Notify,
    finish: Notify,
    completed: Notify,
}
impl Host {
    fn new(data: impl Into<Bytes>) -> Self {
        Self {
            data: Mutex::new(Some(data.into())),
            calls: Mutex::new(Vec::new()),
            gate: None,
            entered: Notify::new(),
            finish: Notify::new(),
            completed: Notify::new(),
        }
    }
}
impl Provider for Host {
    fn allows_tool(&self, id: u32) -> bool {
        (101..=105).contains(&id)
    }
    async fn call_bound_tool(&self, id: u32, args: ToolArguments) -> Result<Reply, TaskError> {
        if self.gate == Some(id) {
            self.entered.notify_one();
            self.finish.notified().await;
        }
        let reply = match args {
            ToolArguments::Numeric(values, count) => {
                self.calls
                    .lock()
                    .unwrap()
                    .push(Call::Numeric(id, values[..count].to_vec()));
                match (id, &values[..count]) {
                    (101, [7]) => Reply::Json(self.data.lock().unwrap().take().unwrap()),
                    (102, [7]) => Reply::Json(Bytes::from_static(
                        br#"{"limit":10,"used":3,"label":"daily"}"#,
                    )),
                    (105, [7, total]) => Reply::I64(100 + total),
                    _ => return Err(TaskError::Permission),
                }
            }
            ToolArguments::String(value) => {
                self.calls
                    .lock()
                    .unwrap()
                    .push(Call::Text(id, value.as_str().to_owned()));
                match id {
                    103 if value.as_str() == "reports/draft-note.txt" => {
                        Reply::Json(Bytes::from_static("{\"content\":\"draft 🌍\"}".as_bytes()))
                    }
                    104 => Reply::I64(7),
                    _ => return Err(TaskError::Permission),
                }
            }
        };
        if self.gate == Some(id) {
            self.completed.notify_one();
        }
        Ok(reply)
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
    if native {
        runtime::execute_script(
            Rc::new(CodeUnit::compile_with_declarations(source, INPUTS, &declarations()).unwrap()),
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
                InterpreterUnit::prepare_with_declarations(source, INPUTS, &declarations())
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
const DATA: &str = r#"{"status":"ready","count":3,"filename":"draft-note.txt","category":"incident","message":"disk error"}"#;
#[tokio::test]
async fn original_cv_scripts_and_branches_both_backends() {
    for native in [false, true] {
        for (index, expected, calls) in [
            (1, "7\ndaily\n", vec![Call::Numeric(102, vec![7])]),
            (
                2,
                "draft 🌍\n",
                vec![
                    Call::Numeric(101, vec![7]),
                    Call::Text(103, "reports/draft-note.txt".into()),
                ],
            ),
            (
                5,
                "104\n",
                vec![Call::Numeric(102, vec![7]), Call::Numeric(105, vec![7, 4])],
            ),
            (
                6,
                "7\n",
                vec![
                    Call::Numeric(101, vec![7]),
                    Call::Text(104, "alerts/draft-note.txt".into()),
                ],
            ),
            (
                7,
                "7\n",
                vec![
                    Call::Numeric(101, vec![7]),
                    Call::Text(104, "disk error".into()),
                ],
            ),
            (
                9,
                "needs attention: disk error\n",
                vec![Call::Numeric(101, vec![7])],
            ),
        ] {
            let host = Arc::new(Host::new(DATA));
            let out = run(
                native,
                CASES[index],
                host.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Ok(()), "case {} backend {native}", index + 1);
            assert_eq!(out.stdout, expected);
            assert_eq!(*host.calls.lock().unwrap(), calls);
        }
        for (index, data, expected, calls) in [
            (
                6,
                DATA.replace("incident", "note"),
                "7\n",
                vec![
                    Call::Numeric(101, vec![7]),
                    Call::Text(104, "archive/draft-note.txt".into()),
                ],
            ),
            (
                7,
                DATA.replace("disk error", ""),
                "no message\n",
                vec![Call::Numeric(101, vec![7])],
            ),
        ] {
            let host = Arc::new(Host::new(data));
            let out = run(
                native,
                CASES[index],
                host.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Ok(()));
            assert_eq!(out.stdout, expected);
            assert_eq!(*host.calls.lock().unwrap(), calls);
        }
    }
}
#[test]
fn bounded_admission_and_remaining_cv_gaps() {
    for index in [3, 4, 8] {
        let a = CodeUnit::compile_with_declarations(CASES[index], INPUTS, &declarations())
            .err()
            .unwrap();
        let b = InterpreterUnit::prepare_with_declarations(CASES[index], INPUTS, &declarations())
            .err()
            .unwrap();
        assert_eq!(a, b);
    }
    for expr in [
        "d['message'] == d['status']",
        "d['message'] < 'a'",
        "'a' < d['message']",
        "d['message'] == 'a' == 'a'",
        "(d['message'] + 'x') == 'ax'",
        "d['count'] == '3'",
        "d['message']",
        "d['message'].startswith('a')",
        "'a' == 'a'",
    ] {
        let source = format!("d=tools.fetch_state(request_id)\nif {expr}:\n    print(1)");
        assert!(
            CodeUnit::compile_with_declarations(&source, INPUTS, &declarations()).is_err(),
            "{expr}"
        );
        assert!(
            InterpreterUnit::prepare_with_declarations(&source, INPUTS, &declarations()).is_err(),
            "{expr}"
        );
    }
    for (len, accepted) in [(4096, true), (4097, false)] {
        let source = format!(
            "d=tools.fetch_state(request_id)\nprint(d['message'] == '{}')",
            "a".repeat(len)
        );
        assert_eq!(
            CodeUnit::compile_with_declarations(&source, INPUTS, &declarations()).is_ok(),
            accepted
        );
        assert_eq!(
            InterpreterUnit::prepare_with_declarations(&source, INPUTS, &declarations()).is_ok(),
            accepted
        );
    }
    for literal in [
        "'\\ud800'",
        "'\\U0000dfff'",
        "'a' '\\ud801'",
        "'\\ud83d\\ude00'",
    ] {
        let source = format!("d=tools.fetch_state(request_id)\nprint(d['message'] == {literal})");
        assert!(CodeUnit::compile_with_declarations(&source, INPUTS, &declarations()).is_err());
        assert!(
            InterpreterUnit::prepare_with_declarations(&source, INPUTS, &declarations()).is_err()
        );
    }
    // Scalar controls keep their existing admission categories.
    assert_eq!(
        CodeUnit::compile_script("print(1 == True)", &[])
            .err()
            .unwrap()
            .kind,
        AdmissionKind::Type
    );
}
#[tokio::test]
async fn unicode_predicates_literals_on_either_side_and_aliases() {
    for native in [false, true] {
        for literal in [
            "'�'",
            "'\\ufffd'",
            "r'\\ud800' '�'",
            "'\\\\ud800�'",
            "'''�'''",
            "'�' # comment\n",
        ] {
            let source =
                format!("d=tools.fetch_state(request_id)\nprint(d['message'] != ({literal}))");
            let out = run(
                native,
                &source,
                Arc::new(Host::new(r#"{"message":""}"#)),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Ok(()), "{literal}");
            assert_eq!(out.stdout, "True\n");
        }
        for (raw, literal, equal) in [
            (r#""""#, "", true),
            (r#""abc""#, "ab", false),
            (r#""你好""#, "你好", true),
            (r#""😀""#, "😀", true),
            (r#""\ud83d\ude00""#, "😀", true),
            (r#""\u00e9""#, "é", true),
            (r#""é""#, "e\\u0301", false),
            (r#""a\u0000b""#, "a\\x00b", true),
        ] {
            let source = format!(
                "d=tools.fetch_state(request_id)\ns=d['message']\nprint(d['message'] == '{literal}')\nprint('{literal}' == s)\nprint(s != '{literal}')\nprint('{literal}' != d['message'])\nprint(not (s == '{literal}'))"
            );
            let out = run(
                native,
                &source,
                Arc::new(Host::new(format!("{{\"message\":{raw}}}"))),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Ok(()), "{raw} {literal} {native}");
            let yes = if equal { "True" } else { "False" };
            let no = if equal { "False" } else { "True" };
            assert_eq!(out.stdout, format!("{yes}\n{yes}\n{no}\n{no}\n{no}\n"));
        }
    }
}
#[tokio::test]
async fn error_order_short_circuit_and_no_later_effect() {
    for native in [false, true] {
        for raw in ["null", "4", "true", "[]", r#""\ud800""#] {
            let source = "d=tools.fetch_state(request_id)\nprint(1)\nprint(d['message'] == '')\nr=tools.publish(request_id,0)";
            let host = Arc::new(Host::new(format!("{{\"message\":{raw}}}")));
            let out = run(
                native,
                source,
                host.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert!(
                matches!(out.status, Err(TaskError::Json(_))),
                "{raw} {:?}",
                out.status
            );
            assert_eq!(out.stdout, "1\n");
            assert_eq!(host.calls.lock().unwrap().len(), 1);
        }
        for bytes in [
            Bytes::from_static(b"{}"),
            Bytes::from_static(b"{\"message\":\"\xff\"}"),
        ] {
            let out = run(
                native,
                "d=tools.fetch_state(request_id)\nprint(d['message'] != '')",
                Arc::new(Host::new(bytes)),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert!(matches!(out.status, Err(TaskError::Json(_))));
        }
        let source = "d=tools.fetch_state(request_id)\nprint(False and d['message'] == '')\nprint(True or '' != d['owner'])";
        let out = run(
            native,
            source,
            Arc::new(Host::new("{}")),
            ScriptLimits {
                string_scan_bytes: 0,
                ..ScriptLimits::default()
            },
            Cancellation::default(),
        )
        .await;
        assert_eq!(out.status, Ok(()));
        assert_eq!(out.stdout, "False\nTrue\n");
        let out = run(
            native,
            "d=tools.fetch_state(request_id)\nprint(1)\ns=d['message']\nprint(False and s == '')",
            Arc::new(Host::new(r#"{"message":null}"#)),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert!(matches!(out.status, Err(TaskError::Json(_))));
        assert_eq!(out.stdout, "1\n");
        // Missing path wins before charge. Located wrong type loses to charge.
        for (data, budget, expected) in [
            (
                "{}",
                0,
                TaskError::Json("missing field or invalid JSON on accessed path"),
            ),
            (r#"{"message":null}"#, 0, TaskError::Budget),
            (
                r#"{"message":null}"#,
                4,
                TaskError::Json("expected UTF-8 string"),
            ),
        ] {
            let out = run(
                native,
                "d=tools.fetch_state(request_id)\nprint(d['message'] == '')",
                Arc::new(Host::new(data)),
                ScriptLimits {
                    string_scan_bytes: budget,
                    ..ScriptLimits::default()
                },
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Err(expected));
        }
    }
}
#[tokio::test]
async fn exact_scan_decode_retained_and_token_budgets() {
    for native in [false, true] {
        let source = "d=tools.fetch_state(request_id)\nprint(d['message'] == 'é')\nprint(d['message'] != 'é')";
        for raw in ["\"é\"", r#""\u00e9""#] {
            let data = format!("{{\"message\":{raw}}}");
            let required =
                2 * (raw.len() + "é".len()) + if raw.contains('\\') { raw.len() } else { 0 };
            for (budget, status) in [(required, Ok(())), (required - 1, Err(TaskError::Budget))] {
                let out = run(
                    native,
                    source,
                    Arc::new(Host::new(data.clone())),
                    ScriptLimits {
                        string_scan_bytes: budget,
                        ..ScriptLimits::default()
                    },
                    Cancellation::default(),
                )
                .await;
                assert_eq!(out.status, status, "{native} {raw} {budget}");
                assert_eq!(
                    out.stdout,
                    if budget == required {
                        "True\nFalse\n"
                    } else {
                        "True\n"
                    }
                );
            }
            if raw.contains('\\') {
                let mut limits = ScriptLimits::default();
                limits.limits.buffer_bytes = data.len();
                let out = run(
                    native,
                    source,
                    Arc::new(Host::new(data)),
                    limits,
                    Cancellation::default(),
                )
                .await;
                assert_eq!(out.status, Err(TaskError::Budget));
            }
        }
        for (len, status) in [
            (64 * 1024 - 2, Ok(())),
            (64 * 1024 - 1, Err(TaskError::Budget)),
        ] {
            let out = run(
                native,
                "d=tools.fetch_state(request_id)\nprint(d['message'] == '')",
                Arc::new(Host::new(format!(
                    "{{\"message\":\"{}\"}}",
                    "x".repeat(len)
                ))),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, status);
            if out.status.is_ok() {
                assert_eq!(out.stdout, "False\n");
            }
        }
    }
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
#[tokio::test]
async fn owner_across_wait_last_use_cancellation_and_detached_argument() {
    for native in [false, true] {
        for raw in [r#""abc""#, r#""\u0061bc""#] {
            for mode in ["resume", "cancel", "last", "detached", "budget"] {
                let live = Arc::new(AtomicUsize::new(1));
                let data = format!("{{\"message\":{raw}}}")
                    .into_bytes()
                    .into_boxed_slice();
                let mut host = Host::new(Bytes::from_owner(Owner {
                    data,
                    live: live.clone(),
                }));
                let detached = mode == "detached";
                host.gate = Some(if detached { 104 } else { 105 });
                let host = Arc::new(host);
                let source = if detached {
                    "d=tools.fetch_state(request_id)\nprint(d['message'] == 'abc')\nr=tools.emit_text(d['message'])"
                } else if mode == "last" {
                    "d=tools.fetch_state(request_id)\nprint(d['message'] == 'abc')\nr=tools.publish(request_id,0)"
                } else {
                    "d=tools.fetch_state(request_id)\nprint(d['message'] == 'abc')\nr=tools.publish(request_id,0)\nprint(d['message'] != 'abc')"
                };
                let cancel = Cancellation::default();
                let limits = ScriptLimits {
                    string_scan_bytes: if mode == "budget" {
                        raw.len() + 3 + if raw.contains('\\') { raw.len() } else { 0 }
                    } else {
                        1024
                    },
                    ..ScriptLimits::default()
                };
                let task = run(native, source, host.clone(), limits, cancel.clone());
                tokio::pin!(task);
                tokio::select! { r = &mut task => panic!("early {:?}", r.status), _ = host.entered.notified() => {} }
                assert_eq!(
                    live.load(Ordering::SeqCst),
                    if mode == "last" || (detached && raw.contains('\\')) {
                        0
                    } else {
                        1
                    },
                    "{native} {raw} {mode}"
                );
                if mode == "cancel" || detached {
                    cancel.cancel();
                    let out = task.await;
                    assert_eq!(out.status, Err(TaskError::Cancelled));
                    assert_eq!(out.stdout, "True\n");
                    assert_eq!(
                        live.load(Ordering::SeqCst),
                        usize::from(detached && !raw.contains('\\'))
                    );
                    host.finish.notify_one();
                    host.completed.notified().await;
                    if detached {
                        assert_eq!(
                            host.calls.lock().unwrap().last(),
                            Some(&Call::Text(104, "abc".into()))
                        );
                    }
                } else {
                    host.finish.notify_one();
                    let out = task.await;
                    assert_eq!(
                        out.status,
                        if mode == "budget" {
                            Err(TaskError::Budget)
                        } else {
                            Ok(())
                        }
                    );
                    assert_eq!(
                        out.stdout,
                        if mode == "resume" {
                            "True\nFalse\n"
                        } else {
                            "True\n"
                        }
                    );
                }
                assert_eq!(live.load(Ordering::SeqCst), 0);
            }
        }
    }
}

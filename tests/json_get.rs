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
        ArgumentSchema, JsonLeaf, Reply, ResultSchema, StringValue, ToolArguments, ToolDeclaration,
    },
};
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};
use tokio::sync::Notify;

fn declarations() -> Vec<ToolDeclaration> {
    vec![
        ToolDeclaration {
            id: 101,
            name: "fetch_state".into(),
            arguments: vec![ArgumentSchema::I64("request_id".into())],
            result: ResultSchema::JsonTyped(vec![
                (vec!["owner".into()], JsonLeaf::String),
                (vec!["meta".into(), "owner".into()], JsonLeaf::String),
                (vec!["other".into()], JsonLeaf::String),
                (vec!["count".into()], JsonLeaf::I64),
            ]),
        },
        ToolDeclaration {
            id: 102,
            name: "checkpoint".into(),
            arguments: vec![ArgumentSchema::I64("request_id".into())],
            result: ResultSchema::I64,
        },
        ToolDeclaration {
            id: 103,
            name: "publish_owner".into(),
            arguments: vec![ArgumentSchema::String("owner".into())],
            result: ResultSchema::I64,
        },
    ]
}
struct Host {
    data: Mutex<Option<Bytes>>,
    calls: Mutex<Vec<String>>,
    allow_publish: bool,
    wait_checkpoint: bool,
    wait_publish: bool,
    entered: Notify,
    finish: Notify,
    done: Notify,
    kind: Mutex<Option<&'static str>>,
}
impl Host {
    fn new(data: impl Into<Bytes>) -> Self {
        Self {
            data: Mutex::new(Some(data.into())),
            calls: Mutex::new(vec![]),
            allow_publish: true,
            wait_checkpoint: false,
            wait_publish: false,
            entered: Notify::new(),
            finish: Notify::new(),
            done: Notify::new(),
            kind: Mutex::new(None),
        }
    }
}
impl Provider for Host {
    fn allows_tool(&self, id: u32) -> bool {
        id != 103 || self.allow_publish
    }
    async fn call_bound_tool(&self, id: u32, args: ToolArguments) -> Result<Reply, TaskError> {
        if id == 101 || id == 102 {
            let ToolArguments::Numeric(values, 1) = args else {
                panic!("numeric argument")
            };
            assert_eq!(values[0], 7);
            self.calls
                .lock()
                .unwrap()
                .push(if id == 101 { "fetch" } else { "checkpoint" }.into());
            if id == 101 {
                return Ok(Reply::Json(self.data.lock().unwrap().take().unwrap()));
            }
            if self.wait_checkpoint {
                self.entered.notify_one();
                self.finish.notified().await;
            }
            return Ok(Reply::I64(8));
        }
        assert_eq!(id, 103);
        let ToolArguments::String(value) = args else {
            panic!("string argument")
        };
        *self.kind.lock().unwrap() = Some(match &value {
            StringValue::Literal(_) => "literal",
            StringValue::Decoded(_) => "decoded",
            StringValue::Json { .. } => "json",
            StringValue::Joined(_) => "joined",
        });
        self.entered.notify_one();
        if self.wait_publish {
            self.finish.notified().await;
        }
        self.calls
            .lock()
            .unwrap()
            .push(format!("publish:{}", value.as_str()));
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
const FETCH: &str = "state = tools.fetch_state(request_id)\n";
const TOOL: &str = "state = tools.fetch_state(request_id)\ncheckpoint = tools.checkpoint(request_id)\nreceipt = tools.publish_owner(state.get(\"owner\", \"unassigned\"))\nprint(receipt)\n";

#[tokio::test]
async fn unchanged_cv05_and_complete_tool_script() {
    let cv = include_str!("../fixtures/cv/05_missing_default.py");
    assert_eq!(
        cv,
        "state = tools.fetch_state(request_id)\nprint(state.get(\"owner\", \"unassigned\"))\n"
    );
    for native in [false, true] {
        for (data, expected) in [
            ("{}", "unassigned"),
            (r#"{"owner":"alice"}"#, "alice"),
            (r#"{"owner":""}"#, ""),
            ("{\"owner\":\"雪🌍\"}", "雪🌍"),
            (r#"{"owner":"h\u00e9\n\""}"#, "hé\n\""),
        ] {
            for (source, stdout, calls) in [
                (cv, format!("{expected}\n"), vec!["fetch".into()]),
                (
                    TOOL,
                    "9\n".into(),
                    vec![
                        "fetch".into(),
                        "checkpoint".into(),
                        format!("publish:{expected}"),
                    ],
                ),
            ] {
                let host = Arc::new(Host::new(data.to_owned()));
                let out = run(
                    native,
                    source,
                    host.clone(),
                    ScriptLimits::default(),
                    Cancellation::default(),
                )
                .await;
                assert_eq!(out.status, Ok(()), "{native} {data}");
                assert_eq!(out.stdout, stdout);
                assert_eq!(*host.calls.lock().unwrap(), calls);
            }
        }
    }
}

#[tokio::test]
async fn shared_sinks_aliases_expression_defaults_and_required_path() {
    for native in [false, true] {
        for data in ["{}", r#"{"owner":"x"}"#] {
            let source = format!(
                "{FETCH}print(state.get('owner','A') + state.get('owner','B'))\nr=tools.publish_owner(owner=f\"[{{state.get('owner','C')}}]\")\nprint(state['owner'])"
            );
            let host = Arc::new(Host::new(data.to_owned()));
            let out = run(
                native,
                &source,
                host.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            if data == "{}" {
                assert!(matches!(out.status, Err(TaskError::Json(_))));
                assert_eq!(out.stdout, "AB\n");
                assert_eq!(*host.calls.lock().unwrap(), ["fetch", "publish:[C]"]);
            } else {
                assert_eq!(out.status, Ok(()));
                assert_eq!(out.stdout, "xx\nx\n");
                assert_eq!(*host.calls.lock().unwrap(), ["fetch", "publish:[x]"]);
            }
        }
        let source = format!(
            "{FETCH}v=state['meta']\nprint('[' + v.get('owner','雪') + ']')\nprint(f\"{{state['meta'].get('owner','B')}}\")"
        );
        let out = run(
            native,
            &source,
            Arc::new(Host::new(r#"{"meta":{}}"#)),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(out.status, Ok(()));
        assert_eq!(out.stdout, "[雪]\nB\n");
        let out = run(
            native,
            &format!("{FETCH}if False:\n print(state.get('owner','u'))\nprint(1)"),
            Arc::new(Host::new("{garbage}")),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(out.status, Ok(()));
        assert_eq!(out.stdout, "1\n");
    }
}

#[tokio::test]
async fn invalid_values_parents_and_missing_proofs_never_fallback_or_submit() {
    let mut bad: Vec<Vec<u8>> = [
        "null",
        "[]",
        "1",
        "false",
        "\"text\"",
        "{garbage}",
        r#"{"other":1,}"#,
        r#"{"other":tru}"#,
        r#"{"other":"\q"}"#,
        r#"{"other":1"#,
        r#"{"owner":null}"#,
        r#"{"owner":1}"#,
        r#"{"owner":false}"#,
        r#"{"owner":{}}"#,
        r#"{"owner":[]}"#,
        r#"{"owner":null,"owner":"last"}"#,
        r#"{"ow\u006eer":null,"owner":"last"}"#,
    ]
    .iter()
    .map(|s| s.as_bytes().to_vec())
    .collect();
    bad.extend([b"{\"other\":\"\xff\"}".to_vec(), b"{\"\xff\":1}".to_vec()]);
    for native in [false, true] {
        for data in &bad {
            for sink in [
                "print('[' + state.get('owner','u') + ']')",
                "r=tools.publish_owner(state.get('owner','u'))",
            ] {
                let host = Arc::new(Host::new(data.clone()));
                let out = run(
                    native,
                    &format!("print(1)\n{FETCH}{sink}"),
                    host.clone(),
                    ScriptLimits::default(),
                    Cancellation::default(),
                )
                .await;
                assert!(
                    matches!(out.status, Err(TaskError::Json(_))),
                    "{native} {data:?} {sink}: {:?}",
                    out.status
                );
                assert_eq!(out.stdout, "1\n");
                assert_eq!(*host.calls.lock().unwrap(), ["fetch"]);
            }
        }
        for data in [
            "{}",
            r#"{"meta":null}"#,
            r#"{"meta":[]}"#,
            r#"{"meta":"text"}"#,
            r#"{"meta":1}"#,
        ] {
            let out = run(
                native,
                &format!("{FETCH}print(state['meta'].get('owner','u'))"),
                Arc::new(Host::new(data)),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert!(matches!(out.status, Err(TaskError::Json(_))), "{data}");
            assert_eq!(out.stdout, "");
        }
        for alias in ["v=state['owner']", "v=state['meta']"] {
            let out = run(
                native,
                &format!("{FETCH}{alias}\nprint(state.get('owner','u'))"),
                Arc::new(Host::new("{}")),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert!(matches!(out.status, Err(TaskError::Json(_))));
            assert_eq!(out.stdout, "");
        }
    }
}

#[tokio::test]
async fn lazy_present_and_selected_receiver_only_with_duplicate_first() {
    for native in [false, true] {
        for (data, expr, expected) in [
            (
                b"{\"owner\":\"yes\",\"bad\":tru}".as_slice(),
                "state.get('owner','u')",
                "yes",
            ),
            (b"{\"owner\":\"yes\"", "state.get('owner','u')", "yes"),
            (
                b"{\"owner\":\"yes\",\"bad\":\"\xff\"}",
                "state.get('owner','u')",
                "yes",
            ),
            (
                b"{\"owner\":\"first\",\"owner\":\"last\"}",
                "state.get('owner','u')",
                "first",
            ),
            (
                br#"{"ow\u006eer":"first","owner":"last"}"#,
                "state.get('owner','u')",
                "first",
            ),
            (b" {} garbage", "state.get('owner','u')", "u"),
            (b"{}\xff", "state.get('owner','u')", "u"),
            (
                br#"{"meta":{},"bad":tru"#,
                "state['meta'].get('owner','u')",
                "u",
            ),
            (br#"{"other":"\uD800"}"#, "state.get('owner','u')", "u"),
        ] {
            let out = run(
                native,
                &format!("{FETCH}print({expr})"),
                Arc::new(Host::new(Bytes::copy_from_slice(data))),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Ok(()), "{native} {data:?}");
            assert_eq!(out.stdout, format!("{expected}\n"));
        }
    }
}

#[tokio::test]
async fn exact_scan_budgets_atomicity_and_error_order() {
    for native in [false, true] {
        for (data, scan, ok) in [
            ("{}", 0, true),
            (r#"{"owner":"x"}"#, 6, true),
            (r#"{"owner":"x"}"#, 5, false),
            (r#"{"owner":"\u0078"}"#, 24, true),
            (r#"{"owner":"\u0078"}"#, 23, false),
        ] {
            for sink in [
                "print(state.get('owner','u') + state.get('owner','v'))",
                "r=tools.publish_owner(state.get('owner','u') + state.get('owner','v'))",
            ] {
                let host = Arc::new(Host::new(data));
                let out = run(
                    native,
                    &format!("{FETCH}{sink}"),
                    host.clone(),
                    ScriptLimits {
                        string_scan_bytes: scan,
                        ..ScriptLimits::default()
                    },
                    Cancellation::default(),
                )
                .await;
                assert_eq!(
                    out.status,
                    if ok { Ok(()) } else { Err(TaskError::Budget) },
                    "{native} {data} {scan}"
                );
                if !ok {
                    assert_eq!(out.stdout, "");
                    assert_eq!(*host.calls.lock().unwrap(), ["fetch"]);
                }
            }
        }
        for (expr, scan, expected) in [
            (
                "state.get('owner','u') + state.get('other','v')",
                0,
                TaskError::Budget,
            ),
            (
                "state.get('other','v') + state.get('owner','u')",
                0,
                TaskError::Budget,
            ),
            (
                "state.get('owner','u') + state.get('other','v')",
                100,
                TaskError::Json("expected UTF-8 string"),
            ),
        ] {
            for sink in [
                format!("print({expr})"),
                format!("r=tools.publish_owner({expr})"),
            ] {
                let mut h = Host::new(r#"{"owner":null,"other":null}"#);
                h.allow_publish = false;
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
        for limits in [
            ScriptLimits {
                stdout_bytes: 2,
                ..ScriptLimits::default()
            },
            ScriptLimits {
                allow_print: false,
                ..ScriptLimits::default()
            },
        ] {
            let out = run(
                native,
                &format!("{FETCH}print(state.get('owner','abc'))"),
                Arc::new(Host::new("{}")),
                limits,
                Cancellation::default(),
            )
            .await;
            assert!(matches!(
                out.status,
                Err(TaskError::Budget | TaskError::Permission)
            ));
            assert_eq!(out.stdout, "");
        }
        let mut h = Host::new("{}");
        h.allow_publish = false;
        let host = Arc::new(h);
        let out = run(
            native,
            TOOL,
            host.clone(),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(out.status, Err(TaskError::Permission));
        assert_eq!(*host.calls.lock().unwrap(), ["fetch", "checkpoint"]);
        let host = Arc::new(Host::new("{}"));
        let out = run(
            native,
            TOOL,
            host.clone(),
            ScriptLimits {
                tool_calls: 2,
                ..ScriptLimits::default()
            },
            Cancellation::default(),
        )
        .await;
        assert_eq!(out.status, Err(TaskError::Budget));
        assert_eq!(*host.calls.lock().unwrap(), ["fetch", "checkpoint"]);
    }
}

#[test]
fn whole_source_admission_and_limits() {
    for (tail, kind) in [
        ("print(state.get('owner'))", AdmissionKind::Unsupported),
        (
            "print(state.get('owner',default='u'))",
            AdmissionKind::Unsupported,
        ),
        (
            "print(state.get('owner',tools.checkpoint(request_id)))",
            AdmissionKind::Unsupported,
        ),
        (
            "print(state.get('owner','a'+'b'))",
            AdmissionKind::Unsupported,
        ),
        (
            "print(state.get(request_id,'u'))",
            AdmissionKind::Unsupported,
        ),
        (
            "print(state.get('owner',state.get('other','u')))",
            AdmissionKind::Unsupported,
        ),
        (
            "if state.get('owner','u') == 'x':\n print(1)",
            AdmissionKind::Unsupported,
        ),
        (
            "print(state.get('owner','u').startswith('a'))",
            AdmissionKind::Unsupported,
        ),
        (
            "print(state.get('\\ud800','u'))",
            AdmissionKind::Unsupported,
        ),
        ("v=state.get('owner','u')", AdmissionKind::Unsupported),
        ("print(state.get('unknown','u'))", AdmissionKind::Schema),
        ("print(state.get('count','u'))", AdmissionKind::Type),
        (
            "if False:\n print(state.get('owner','\\ud800'))",
            AdmissionKind::Unsupported,
        ),
    ] {
        let source = format!("{FETCH}{tail}");
        let e = frontend::parse_with_declarations(
            &source,
            &[("request_id", Type::I64)],
            &declarations(),
        )
        .unwrap_err();
        assert_eq!(e.kind, kind, "{tail}: {e}");
        assert!(
            CodeUnit::compile_with_declarations(
                &source,
                &[("request_id", Type::I64)],
                &declarations()
            )
            .is_err()
        );
        assert!(
            InterpreterUnit::prepare_with_declarations(
                &source,
                &[("request_id", Type::I64)],
                &declarations()
            )
            .is_err()
        );
    }
    for n in [4096, 4097] {
        let source = format!("{FETCH}print(state.get('owner','{}'))", "x".repeat(n));
        let result = frontend::parse_with_declarations(
            &source,
            &[("request_id", Type::I64)],
            &declarations(),
        );
        if n == 4096 {
            assert!(result.is_ok());
        } else {
            assert_eq!(result.unwrap_err().kind, AdmissionKind::Resource);
        }
    }
    for n in [32, 33] {
        let source = format!(
            "{FETCH}print({})",
            vec!["state.get('owner','u')"; n].join(" + ")
        );
        let result = frontend::parse_with_declarations(
            &source,
            &[("request_id", Type::I64)],
            &declarations(),
        );
        if n == 32 {
            assert!(result.is_ok());
        } else {
            assert_eq!(result.unwrap_err().kind, AdmissionKind::Resource);
        }
    }
    let source = format!(
        "{FETCH}print(state.get('owner','u'))\n#{}",
        "x".repeat(16 * 1024)
    );
    assert_eq!(
        frontend::parse_with_declarations(&source, &[("request_id", Type::I64)], &declarations())
            .unwrap_err()
            .kind,
        AdmissionKind::Resource
    );
}

#[tokio::test]
async fn token_join_retained_bounds_and_piece_priority() {
    for native in [false, true] {
        for (len, expected) in [(65534, Ok(())), (65535, Err(TaskError::Budget))] {
            let source = format!("{FETCH}print(state.get('owner','u'))");
            let data = format!(r#"{{"owner":"{}"}}"#, "x".repeat(len));
            let out = run(
                native,
                &source,
                Arc::new(Host::new(data)),
                ScriptLimits {
                    stdout_bytes: 70000,
                    ..ScriptLimits::default()
                },
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, expected);
            assert_eq!(out.stdout.len(), if len == 65534 { len + 1 } else { 0 });
        }
        for (expr, expected) in [
            (
                "state.get('owner','u') + state.get('owner','v') + state.get('other','w')",
                TaskError::Budget,
            ),
            (
                "state.get('other','w') + state.get('owner','u') + state.get('owner','v')",
                TaskError::Json("expected UTF-8 string"),
            ),
        ] {
            for sink in [
                format!("print({expr})"),
                format!("r=tools.publish_owner({expr})"),
            ] {
                let host = Arc::new(Host::new(format!(
                    r#"{{"owner":"{}","other":null}}"#,
                    "x".repeat(40000)
                )));
                let out = run(
                    native,
                    &format!("print(1)\n{FETCH}{sink}"),
                    host.clone(),
                    ScriptLimits {
                        stdout_bytes: 100000,
                        ..ScriptLimits::default()
                    },
                    Cancellation::default(),
                )
                .await;
                assert_eq!(out.status, Err(expected.clone()));
                assert_eq!(out.stdout, "1\n");
                assert_eq!(*host.calls.lock().unwrap(), ["fetch"]);
            }
        }
        let data = r#"{"owner":"\u0078"}"#;
        for sink in [
            "print(state.get('owner','u'))",
            "r=tools.publish_owner(state.get('owner','u'))",
        ] {
            let host = Arc::new(Host::new(data));
            let out = run(
                native,
                &format!("{FETCH}{sink}"),
                host.clone(),
                ScriptLimits {
                    limits: runtime::Limits {
                        buffer_bytes: data.len(),
                        ..runtime::Limits::default()
                    },
                    ..ScriptLimits::default()
                },
                Cancellation::default(),
            )
            .await;
            assert_eq!(out.status, Err(TaskError::Budget));
            assert_eq!(out.stdout, "");
            assert_eq!(*host.calls.lock().unwrap(), ["fetch"]);
        }
        // Located null precedes final-key lookup in the later, invalid receiver.
        for expr in [
            "state.get('owner','u') + state['meta'].get('owner','v')",
            "state['meta'].get('owner','v') + state.get('owner','u')",
        ] {
            let out = run(
                native,
                &format!("{FETCH}print({expr})"),
                Arc::new(Host::new(r#"{"owner":null,"meta":null}"#)),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(
                out.status,
                Err(TaskError::Json(if expr.starts_with("state.get") {
                    "expected UTF-8 string"
                } else {
                    "get receiver must be an object"
                }))
            );
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
async fn real_wait_root_liveness_and_cancelled_operation_owns_argument() {
    for native in [false, true] {
        for (data, expected, kind, headroom) in [
            ("{}".to_owned(), "unassigned", "literal", 1000),
            (r#"{"owner":"x"}"#.to_owned(), "x", "json", 1000),
            (r#"{"owner":"\u0078"}"#.to_owned(), "x", "decoded", 1000),
            (
                format!(r#"{{"owner":"x","padding":"{}"}}"#, "z".repeat(70_000)),
                "x",
                "json",
                1000,
            ),
            (
                format!(r#"{{"owner":"x","padding":"{}"}}"#, "z".repeat(70_000)),
                "x",
                "json",
                0,
            ),
        ] {
            let backing: Arc<[u8]> = Arc::from(data.as_bytes());
            let weak = Arc::downgrade(&backing);
            let mut h = Host::new(Bytes::from_owner(Owned(backing)));
            h.wait_checkpoint = true;
            h.wait_publish = true;
            let host = Arc::new(h);
            let cancel = Cancellation::default();
            let task = run(
                native,
                TOOL,
                host.clone(),
                ScriptLimits {
                    limits: runtime::Limits {
                        buffer_bytes: data.len() + headroom,
                        ..runtime::Limits::default()
                    },
                    ..ScriptLimits::default()
                },
                cancel.clone(),
            );
            let control = async {
                host.entered.notified().await; // checkpoint really suspended
                assert!(weak.upgrade().is_some());
                host.finish.notify_one();
                host.entered.notified().await; // publish really suspended
                assert_eq!(*host.kind.lock().unwrap(), Some(kind));
                let holds_root = kind == "json" && (data.len() < 64 * 1024 || headroom == 0);
                assert_eq!(weak.upgrade().is_some(), holds_root);
                cancel.cancel();
            };
            let (out, ()) = tokio::join!(task, control);
            assert_eq!(out.status, Err(TaskError::Cancelled));
            assert_eq!(out.stdout, "");
            assert_eq!(*host.calls.lock().unwrap(), ["fetch", "checkpoint"]);
            host.finish.notify_one();
            host.done.notified().await;
            tokio::task::yield_now().await;
            assert_eq!(
                *host.calls.lock().unwrap(),
                ["fetch", "checkpoint", &format!("publish:{expected}")]
            );
            assert!(weak.upgrade().is_none());
        }
    }
}

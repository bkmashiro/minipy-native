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
    tool::{JsonLeaf, Reply, ResultSchema, ToolSchema},
};
use std::rc::Rc;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

fn schema() -> Vec<ToolSchema> {
    vec![
        ToolSchema {
            id: 101,
            name: "fetch".into(),
            arguments: vec!["batch_id".into()],
            result: ResultSchema::JsonTyped(vec![
                (vec!["message".into()], JsonLeaf::String),
                (vec!["bad".into()], JsonLeaf::String),
                (vec!["count".into()], JsonLeaf::I64),
            ]),
        },
        ToolSchema {
            id: 102,
            name: "publish".into(),
            arguments: vec!["batch_id".into()],
            result: ResultSchema::I64,
        },
    ]
}
struct P {
    data: Mutex<Option<Bytes>>,
    calls: AtomicUsize,
    gate: bool,
    started: tokio::sync::Notify,
    proceed: tokio::sync::Notify,
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
impl P {
    fn new(data: impl Into<Bytes>) -> Self {
        Self {
            data: Mutex::new(Some(data.into())),
            calls: AtomicUsize::new(0),
            gate: false,
            started: tokio::sync::Notify::new(),
            proceed: tokio::sync::Notify::new(),
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
        id == 101 || id == 102
    }
    async fn call_tool(&self, id: u32, _: [i64; 2], _: usize) -> Result<Reply, TaskError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if id == 101 {
            Ok(Reply::Json(self.data.lock().unwrap().take().unwrap()))
        } else {
            self.started.notify_one();
            if self.gate {
                self.proceed.notified().await;
            }
            Ok(Reply::I64(42))
        }
    }
}
async fn run(
    native: bool,
    source: &str,
    provider: Arc<P>,
    limits: ScriptLimits,
    cancel: Cancellation,
) -> ScriptOutcome {
    let inputs = &[("batch_id", Type::I64)];
    if native {
        runtime::execute_script(
            Rc::new(CodeUnit::compile_with_tools(source, inputs, &schema()).unwrap()),
            provider,
            &[7],
            limits,
            cancel,
            None,
        )
        .await
    } else {
        interpreter::execute_script(
            Rc::new(InterpreterUnit::prepare_with_tools(source, inputs, &schema()).unwrap()),
            provider,
            &[7],
            limits,
            cancel,
            None,
        )
        .await
    }
}
const SOURCE: &str =
    "data = tools.fetch(batch_id)\ntext = data[\"message\"]\nprint(\"error\" in text)\n";
#[tokio::test]
async fn real_source_differential() {
    for native in [false, true] {
        for (data, needle, expected) in [
            (r#"{"message":"error here"}"#, "error", "True\n"),
            (r#"{"message":"a long error"}"#, "error", "True\n"),
            (r#"{"message":"oops"}"#, "error", "False\n"),
            (r#"{"message":"\u0065rror"}"#, "error", "True\n"),
            (r#"{"message":"你好错误"}"#, "错误", "True\n"),
            (r#"{"message":"abc"}"#, "", "True\n"),
            (
                r#"{"message":"first","message":"error"}"#,
                "error",
                "False\n",
            ),
        ] {
            let source = SOURCE.replace("error\" in", &format!("{needle}\" in"));
            let r = run(
                native,
                &source,
                Arc::new(P::new(Bytes::copy_from_slice(data.as_bytes()))),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(r.status, Ok(()), "{data} {native}");
            assert_eq!(r.stdout, expected, "{data} {native}");
            let oracle: serde_json::Value = serde_json::from_str(data).unwrap();
            if !data.contains("\",\"message\"") {
                assert_eq!(
                    r.stdout == "True\n",
                    oracle["message"].as_str().unwrap().contains(needle)
                );
            }
        }
        let r = run(native,
            "d = tools.fetch(batch_id)\ns = d[\"message\"]\nprint(\"error\" in s)\nprint(\"rr\" in s)",
            Arc::new(P::new(Bytes::from_static(br#"{"message":"\u0065rror"}"#))),
            ScriptLimits::default(), Cancellation::default()).await;
        assert_eq!(r.status, Ok(()));
        assert_eq!(r.stdout, "True\nTrue\n");
        for data in [
            r#"{}"#,
            r#"{"message":null}"#,
            r#"{"message":4}"#,
            r#"{"message":true}"#,
            r#"{"message":[]}"#,
            r#"{"message":"\uD800"}"#,
        ] {
            let r = run(
                native,
                SOURCE,
                Arc::new(P::new(Bytes::copy_from_slice(data.as_bytes()))),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert!(
                matches!(r.status, Err(TaskError::Json(_))),
                "{data} {:?}",
                r.status
            );
        }
        let mut invalid = b"{\"message\":\"text\"}".to_vec();
        invalid[13] = 0xff;
        let r = run(
            native,
            SOURCE,
            Arc::new(P::new(Bytes::from(invalid))),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert!(matches!(r.status, Err(TaskError::Json(_))));
        let r = run(native, "d = tools.fetch(batch_id)\nprint(False and \"x\" in d[\"bad\"])\nprint(True or \"x\" in d[\"bad\"])", Arc::new(P::new(Bytes::from_static(br#"{"bad":null}"#))), ScriptLimits::default(), Cancellation::default()).await;
        assert_eq!(r.status, Ok(()));
        assert_eq!(r.stdout, "False\nTrue\n");
        let r = run(
            native,
            "d = tools.fetch(batch_id)\nprint(1)\nprint(\"x\" in d[\"bad\"])",
            Arc::new(P::new(Bytes::from_static(br#"{"bad":null}"#))),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert!(matches!(r.status, Err(TaskError::Json(_))));
        assert_eq!(r.stdout, "1\n");
        let r = run(
            native,
            "d = tools.fetch(batch_id)\nprint(d[\"count\"])\nprint(\"error\" in d[\"message\"])",
            Arc::new(P::new(Bytes::from_static(
                br#"{"count":7,"message":"error"}"#,
            ))),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(r.status, Ok(()));
        assert_eq!(r.stdout, "7\nTrue\n");
    }
}
#[tokio::test]
async fn budgets_and_wait_lifetime() {
    for native in [false, true] {
        let data = Bytes::from_static(br#"{"message":"error"}"#);
        let limits = ScriptLimits {
            string_scan_bytes: 2,
            ..ScriptLimits::default()
        };
        let r = run(
            native,
            SOURCE,
            Arc::new(P::new(data.clone())),
            limits,
            Cancellation::default(),
        )
        .await;
        assert_eq!(r.status, Err(TaskError::Budget));
        let mut limits = ScriptLimits::default();
        limits.limits.buffer_bytes = data.len() - 1;
        let r = run(
            native,
            SOURCE,
            Arc::new(P::new(data.clone())),
            limits,
            Cancellation::default(),
        )
        .await;
        assert_eq!(r.status, Err(TaskError::Budget));
        let source = "d = tools.fetch(batch_id)\nprint(\"z\" in d[\"message\"])";
        for (length, expected) in [
            (64 * 1024 - 2, Ok(())),
            (64 * 1024 - 1, Err(TaskError::Budget)),
        ] {
            let payload = format!("{{\"message\":\"{}\"}}", "a".repeat(length));
            let r = run(
                native,
                source,
                Arc::new(P::new(Bytes::from(payload))),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            assert_eq!(r.status, expected, "{native} {length}");
        }
        let escaped = Bytes::from_static(br#"{"message":"\u0065rror"}"#);
        let mut limits = ScriptLimits::default();
        limits.limits.buffer_bytes = escaped.len();
        let r = run(
            native,
            SOURCE,
            Arc::new(P::new(escaped)),
            limits,
            Cancellation::default(),
        )
        .await;
        assert_eq!(
            r.status,
            Err(TaskError::Budget),
            "decoded scratch shares the root cap"
        );
        let source = "d = tools.fetch(batch_id)\ns = d[\"message\"]\nr = tools.publish(batch_id)\nprint(\"error\" in s)";
        let done = run(
            native,
            source,
            Arc::new(P::new(data.clone())),
            ScriptLimits::default(),
            Cancellation::default(),
        )
        .await;
        assert_eq!(done.status, Ok(()));
        assert_eq!(done.stdout, "True\n");
        let live = Arc::new(AtomicUsize::new(1));
        let owner = Owner {
            data: data.to_vec().into_boxed_slice(),
            live: live.clone(),
        };
        let mut provider = P::new(Bytes::from_owner(owner));
        provider.gate = true;
        let p = Arc::new(provider);
        let cancel = Cancellation::default();
        let task = run(
            native,
            source,
            p.clone(),
            ScriptLimits::default(),
            cancel.clone(),
        );
        tokio::pin!(task);
        tokio::select! { r = &mut task => panic!("early {:?}", r.status), _ = p.started.notified() => {} }
        assert_eq!(p.calls.load(Ordering::SeqCst), 2);
        assert_eq!(
            live.load(Ordering::SeqCst),
            1,
            "string alias holds root across wait"
        );
        cancel.cancel();
        assert_eq!(task.await.status, Err(TaskError::Cancelled));
        assert_eq!(live.load(Ordering::SeqCst), 0, "cancel drops borrowed root");
        p.proceed.notify_one();
    }
}
#[test]
fn schema_rejects_unapproved_shapes() {
    let inputs = &[("batch_id", Type::I64)];
    for source in [
        "print(\"a\" in \"abc\")",
        "d = tools.fetch(batch_id)\nprint(d[\"message\"])",
        "d = tools.fetch(batch_id)\nprint(7 in d[\"message\"])",
        "d = tools.fetch(batch_id)\nprint(\"x\" in d[\"count\"])",
        "d = tools.fetch(batch_id)\nprint(d[\"message\"].split(\"x\"))",
    ] {
        assert!(
            CodeUnit::compile_with_tools(source, inputs, &schema()).is_err(),
            "{source}"
        );
        assert!(
            InterpreterUnit::prepare_with_tools(source, inputs, &schema()).is_err(),
            "{source}"
        );
    }
}

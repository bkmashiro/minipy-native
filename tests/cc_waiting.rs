//! Controlled concurrent waiting payload census, not an RSS or throughput benchmark.
#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
use bytes::Bytes;
use minipy_native::{
    buffer::QuoteBuffer,
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
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

struct Backing {
    bytes: Option<Box<[u8]>>,
    released: Arc<AtomicUsize>,
}
impl AsRef<[u8]> for Backing {
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_ref().unwrap()
    }
}
impl Drop for Backing {
    fn drop(&mut self) {
        drop(self.bytes.take());
        self.released.fetch_add(1, Ordering::SeqCst);
    }
}
struct Host {
    reply: Mutex<Option<Bytes>>,
    entered: Notify,
    finish: Notify,
    completed: Notify,
    held: AtomicUsize,
    expected: String,
}
impl Provider for Host {
    fn allows_tool(&self, _: u32) -> bool {
        true
    }
    async fn call_bound_tool(&self, id: u32, arg: ToolArguments) -> Result<Reply, TaskError> {
        if id == 101 {
            return Ok(Reply::Json(self.reply.lock().unwrap().take().unwrap()));
        }
        let ToolArguments::String(arg) = arg else {
            panic!("expected string")
        };
        let held = match &arg {
            minipy_native::tool::StringValue::Json { owner, .. } => owner.len(),
            minipy_native::tool::StringValue::Decoded(s) => s.capacity(),
            _ => panic!("expected JSON or decoded string"),
        };
        self.held.store(held, Ordering::SeqCst);
        self.entered.notify_one();
        self.finish.notified().await;
        assert_eq!(arg.as_str(), self.expected);
        drop(arg);
        self.completed.notify_one();
        Ok(Reply::I64(7))
    }
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        Err(TaskError::Permission)
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
}

async fn cohort(native: bool, mode: &str, candidate: bool) {
    const N: usize = 8;
    let expected = if mode == "long" {
        "x".repeat(4097)
    } else {
        "report.txt".into()
    };
    let encoded = if mode == "escaped" {
        "report\\u002etxt"
    } else {
        &expected
    };
    let raw = format!(
        "{{\"nested\":{{\"path\":\"{encoded}\"}},\"padding\":\"{}\"}}",
        "x".repeat(if mode == "small" { 128 } else { 1_048_576 })
    );
    let root_bytes = raw.len();
    let released = Arc::new(AtomicUsize::new(0));
    let tools = vec![
        ToolDeclaration {
            id: 101,
            name: "source".into(),
            arguments: vec![],
            result: ResultSchema::JsonTyped(vec![(
                vec!["nested".into(), "path".into()],
                JsonLeaf::String,
            )]),
        },
        ToolDeclaration {
            id: 102,
            name: "consume".into(),
            arguments: vec![ArgumentSchema::String("text".into())],
            result: ResultSchema::I64,
        },
    ];
    let source = format!(
        "d=tools.source()\na=d\nb=a['nested']\ns=b['path']\nr=tools.consume(s)\nprint(r){}",
        if mode == "live" { "\nprint(s)" } else { "" }
    );
    let native_unit =
        native.then(|| Rc::new(CodeUnit::compile_with_declarations(&source, &[], &tools).unwrap()));
    let tape_unit = (!native).then(|| {
        Rc::new(InterpreterUnit::prepare_with_declarations(&source, &[], &tools).unwrap())
    });
    let mut hosts = Vec::new();
    let mut tasks = Vec::new();
    let mut cancels = Vec::new();
    for _ in 0..N {
        let host = Arc::new(Host {
            reply: Mutex::new(Some(Bytes::from_owner(Backing {
                bytes: Some(raw.as_bytes().into()),
                released: released.clone(),
            }))),
            entered: Notify::new(),
            finish: Notify::new(),
            completed: Notify::new(),
            held: AtomicUsize::new(0),
            expected: expected.clone(),
        });
        let mut limits = ScriptLimits::default();
        if mode == "tight" {
            limits.limits.buffer_bytes = root_bytes;
        }
        let cancel = Cancellation::default();
        let (p, c, n, t) = (
            host.clone(),
            cancel.clone(),
            native_unit.clone(),
            tape_unit.clone(),
        );
        tasks.push(Some(tokio::task::spawn_local(async move {
            if let Some(unit) = n {
                runtime::execute_script(unit, p, &[], limits, c, None).await
            } else {
                interpreter::execute_script(t.unwrap(), p, &[], limits, c, None).await
            }
        })));
        cancels.push(cancel);
        hosts.push(host);
    }
    // All N operations are simultaneously blocked before taking the census.
    for host in &hosts {
        host.entered.notified().await;
    }
    let detached = mode == "escaped" || (candidate && mode == "dead");
    let released_before_wait = released.load(Ordering::SeqCst);
    assert_eq!(released_before_wait, if detached { N } else { 0 });
    let held: usize = hosts.iter().map(|h| h.held.load(Ordering::SeqCst)).sum();
    if mode != "escaped" {
        assert_eq!(
            held,
            N * if candidate && mode == "dead" {
                expected.len()
            } else {
                root_bytes
            }
        );
    }
    println!(
        "CC_WAIT {}",
        serde_json::json!({"candidate":candidate,"native":native,"mode":mode,"concurrency":N,"source_payload_bytes":N*root_bytes,"deallocated_source_payload_bytes":released_before_wait*root_bytes,"waiting_argument_payload_or_capacity_bytes":held,"copied_payload_bytes":if candidate && mode == "dead" {N*expected.len()} else {0}})
    );
    // Half cancel while their independently owned operations remain blocked.
    for i in (0..N).step_by(2) {
        cancels[i].cancel();
        assert_eq!(
            tasks[i].take().unwrap().await.unwrap().status,
            Err(TaskError::Cancelled)
        );
    }
    assert_eq!(
        released.load(Ordering::SeqCst),
        if detached { N } else { 0 }
    );
    for host in &hosts {
        host.finish.notify_one();
    }
    for host in &hosts {
        host.completed.notified().await;
    }
    for task in tasks.into_iter().flatten() {
        let out = task.await.unwrap();
        assert_eq!(out.status, Ok(()));
        assert_eq!(
            out.stdout,
            if mode == "live" {
                format!("7\n{expected}\n")
            } else {
                "7\n".into()
            }
        );
    }
    assert_eq!(released.load(Ordering::SeqCst), N);
}

#[tokio::test(flavor = "current_thread")]
async fn concurrent_waiting_payload() {
    // Baseline replay opts out; ordinary regression runs expect production copy.
    let candidate = std::env::var("CC_CANDIDATE").as_deref() != Ok("0");
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        tokio::task::LocalSet::new().run_until(async {
            for native in [false, true] {
                for mode in ["dead", "live", "tight", "long", "small", "escaped"] {
                    cohort(native, mode, candidate).await;
                }
            }
        }),
    )
    .await
    .expect("controlled waiting census must finish");
}

#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
use bytes::Bytes;
use minipy_native::{
    buffer::QuoteBuffer,
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
    tool::{
        ArgumentSchema, JsonLeaf, Reply, ResultSchema, StringValue, ToolArguments, ToolDeclaration,
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

struct Owned {
    bytes: Option<Box<[u8]>>,
    released: Arc<AtomicUsize>,
}
impl AsRef<[u8]> for Owned {
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_ref().unwrap()
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        // Free the actual backing before reporting release. A Weak<Arc<[u8]>>
        // would retain its allocation and cannot prove physical release.
        drop(self.bytes.take());
        self.released.store(1, Ordering::SeqCst);
    }
}
struct Host {
    reply: Mutex<Option<Bytes>>,
    entered: Notify,
    finish: Notify,
    held: AtomicUsize,
    done: AtomicUsize,
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
            panic!("string binding")
        };
        let held = match &arg {
            StringValue::Json { owner, .. } => owner.len(),
            StringValue::Decoded(s) => s.capacity(),
            _ => panic!("unexpected literal"),
        };
        self.held.store(held, Ordering::SeqCst);
        self.entered.notify_one();
        self.finish.notified().await;
        // Read after guest cancellation: the detached operation still owns it.
        assert_eq!(arg.as_str(), self.expected);
        drop(arg);
        self.done.store(1, Ordering::SeqCst);
        Ok(Reply::I64(7))
    }
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        Err(TaskError::Permission)
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
}
async fn exercise(candidate: bool) {
    for native in [false, true] {
        for mode in ["dead", "live", "escaped", "tight", "large", "small"] {
            let expected = if mode == "large" {
                "x".repeat(5000)
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
                "x".repeat(if mode == "small" { 0 } else { 1_048_576 })
            );
            let backing = raw.into_bytes().into_boxed_slice();
            let root_bytes = backing.len();
            let released = Arc::new(AtomicUsize::new(0));
            let host = Arc::new(Host {
                reply: Mutex::new(Some(Bytes::from_owner(Owned {
                    bytes: Some(backing),
                    released: released.clone(),
                }))),
                entered: Notify::new(),
                finish: Notify::new(),
                held: AtomicUsize::new(0),
                done: AtomicUsize::new(0),
                expected,
            });
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
            let mut limits = ScriptLimits::default();
            if mode == "tight" {
                limits.limits.buffer_bytes = root_bytes;
            }
            let cancel = Cancellation::default();
            let task = async {
                if native {
                    runtime::execute_script(
                        Rc::new(CodeUnit::compile_with_declarations(&source, &[], &tools).unwrap()),
                        host.clone(),
                        &[],
                        limits,
                        cancel.clone(),
                        None,
                    )
                    .await
                } else {
                    interpreter::execute_script(
                        Rc::new(
                            InterpreterUnit::prepare_with_declarations(&source, &[], &tools)
                                .unwrap(),
                        ),
                        host.clone(),
                        &[],
                        limits,
                        cancel.clone(),
                        None,
                    )
                    .await
                }
            };
            println!(
                "BZ_LAYOUT {}",
                serde_json::json!({"native":native,"mode":mode,"wrapper_future_bytes":std::mem::size_of_val(&task)})
            );
            tokio::pin!(task);
            tokio::select! { _ = host.entered.notified() => {}, out = &mut task => panic!("unexpected {:?}", out.status) }
            let detached = mode == "escaped" || (candidate && mode == "dead");
            assert_eq!(
                released.load(Ordering::SeqCst) == 1,
                detached,
                "native={native}, mode={mode}"
            );
            let held = host.held.load(Ordering::SeqCst);
            if candidate && mode == "dead" {
                assert_eq!(held, host.expected.len());
            }
            if !detached {
                assert_eq!(held, root_bytes);
            }
            println!(
                "BZ {}",
                serde_json::json!({"native":native,"mode":mode,"root_bytes":root_bytes,"waiting_argument_owner_bytes":held,"backing_released_before_wait":released.load(Ordering::SeqCst) == 1,"copied_bytes":if candidate && mode == "dead" {host.expected.len()} else {0}})
            );
            cancel.cancel();
            assert_eq!(task.await.status, Err(TaskError::Cancelled));
            assert_eq!(host.done.load(Ordering::SeqCst), 0);
            // Live roots now drop with the cancelled guest, but the original
            // allocation remains in uncopied detached arguments until completion.
            assert_eq!(released.load(Ordering::SeqCst) == 1, detached);
            host.finish.notify_one();
            for _ in 0..20 {
                if host.done.load(Ordering::SeqCst) != 0 {
                    break;
                }
                tokio::task::yield_now().await;
            }
            assert_eq!(host.done.load(Ordering::SeqCst), 1);
            assert_eq!(released.load(Ordering::SeqCst), 1);
        }
    }
}
#[tokio::test]
async fn root_detachment_regression() {
    exercise(true).await;
}
#[tokio::test]
#[ignore = "separate ownership diagnostic, not formal latency"]
async fn root_detachment_diagnostic() {
    exercise(std::env::var("BZ_CANDIDATE").as_deref() == Ok("1")).await;
}

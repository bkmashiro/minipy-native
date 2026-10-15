//! Bounded AR operation probe. Real source/Tool execution, no mocked native path.
//! Usage: cargo run --release --offline --manifest-path bench/Cargo.toml \
//!   --features string-probe --bin ar-contains -- [reps]
use bytes::Bytes;
use minipy_native::{
    buffer::QuoteBuffer,
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    ir::Type,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
    tool::{JsonLeaf, Reply, ResultSchema, ToolSchema},
};
use monty::{MontyRun, RunProgress};
use monty_types::{CompileOptions, ExtFunctionResult, MontyObject, PrintWriter, ResourceTracker};
use std::{rc::Rc, sync::Arc, time::Instant};

const NATIVE: &str =
    "data = tools.fetch(batch_id)\ntext = data[\"message\"]\nprint(\"error\" in text)\n";
const MONTY: &str =
    "data = await fetch(batch_id)\ntext = data[\"message\"]\nprint(\"error\" in text)\n";
const HOST: &str = "hit = await contains_message(batch_id)\nprint(hit)\n";
fn schema() -> [ToolSchema; 1] {
    [ToolSchema {
        id: 101,
        name: "fetch".into(),
        arguments: vec!["batch_id".into()],
        result: ResultSchema::JsonTyped(vec![(vec!["message".into()], JsonLeaf::String)]),
    }]
}
struct P {
    bytes: Bytes,
}
impl Provider for P {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        Err(TaskError::Permission)
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
    fn allows_tool(&self, id: u32) -> bool {
        id == 101
    }
    async fn call_tool(&self, _: u32, _: [i64; 2], _: usize) -> Result<Reply, TaskError> {
        Ok(Reply::Json(self.bytes.clone()))
    }
}
fn monty_compile(host: bool) -> MontyRun {
    MontyRun::new(
        if host { HOST } else { MONTY }.into(),
        "probe.py",
        vec![
            "batch_id".into(),
            if host { "contains_message" } else { "fetch" }.into(),
        ],
        CompileOptions::default(),
    )
    .unwrap()
}
async fn monty_run(code: MontyRun, provider: Arc<P>, host: bool) -> String {
    let mut stdout = String::new();
    let mut progress = code
        .start(
            vec![
                MontyObject::int(7),
                MontyObject::function(if host { "contains_message" } else { "fetch" }, None),
            ],
            ResourceTracker::default(),
            PrintWriter::CollectString(&mut stdout, Some(1024)),
        )
        .unwrap();
    loop {
        match progress {
            RunProgress::Complete(_) => return stdout,
            RunProgress::FunctionCall(call) => {
                assert_eq!(call.args.arg(0).and_then(|x| x.as_int()), Some(7));
                let id = call.call_id;
                let RunProgress::ResolveFutures(wait) = call
                    .resume_pending(PrintWriter::CollectString(&mut stdout, Some(1024)))
                    .unwrap()
                else {
                    panic!("no await");
                };
                let (tx, rx) = tokio::sync::oneshot::channel();
                let p = provider.clone();
                tokio::spawn(async move {
                    let parsed: serde_json::Value = serde_json::from_slice(&p.bytes).unwrap();
                    let text = parsed["message"].as_str().unwrap();
                    let value = if host {
                        MontyObject::bool(text.contains("error"))
                    } else {
                        MontyObject::dict([(
                            MontyObject::string("message"),
                            MontyObject::string(text),
                        )])
                    };
                    let _ = tx.send(value);
                });
                let value = rx.await.unwrap();
                progress = wait
                    .resume(
                        vec![(id, ExtFunctionResult::Return(value))],
                        PrintWriter::CollectString(&mut stdout, Some(1024)),
                    )
                    .unwrap();
            }
            _ => panic!("unexpected progress"),
        }
    }
}
async fn once(engine: &str, mode: &str, p: Arc<P>, prepared: &Prepared) -> (u128, String) {
    let start = Instant::now();
    let stdout = match engine {
        "native" => {
            let code = if mode == "cold" {
                Rc::new(
                    CodeUnit::compile_with_tools(NATIVE, &[("batch_id", Type::I64)], &schema())
                        .unwrap(),
                )
            } else {
                prepared.native.as_ref().unwrap().clone()
            };
            let out = runtime::execute_script(
                code,
                p,
                &[7],
                ScriptLimits::default(),
                Cancellation::default(),
                None,
            )
            .await;
            out.status.unwrap();
            out.stdout
        }
        "tape" => {
            let code = if mode == "cold" {
                Rc::new(
                    InterpreterUnit::prepare_with_tools(
                        NATIVE,
                        &[("batch_id", Type::I64)],
                        &schema(),
                    )
                    .unwrap(),
                )
            } else {
                prepared.tape.as_ref().unwrap().clone()
            };
            let out = interpreter::execute_script(
                code,
                p,
                &[7],
                ScriptLimits::default(),
                Cancellation::default(),
                None,
            )
            .await;
            out.status.unwrap();
            out.stdout
        }
        "monty" | "host-bool" => {
            monty_run(
                if mode == "cold" {
                    monty_compile(engine == "host-bool")
                } else if engine == "monty" {
                    prepared.monty.as_ref().unwrap().clone()
                } else {
                    prepared.host.as_ref().unwrap().clone()
                },
                p,
                engine == "host-bool",
            )
            .await
        }
        _ => panic!("engine"),
    };
    (start.elapsed().as_nanos(), stdout)
}
struct Prepared {
    native: Option<Rc<CodeUnit>>,
    tape: Option<Rc<InterpreterUnit>>,
    monty: Option<MontyRun>,
    host: Option<MontyRun>,
}
fn prepared(mode: &str) -> Prepared {
    if mode == "cold" {
        return Prepared {
            native: None,
            tape: None,
            monty: None,
            host: None,
        };
    }
    Prepared {
        native: Some(Rc::new(
            CodeUnit::compile_with_tools(NATIVE, &[("batch_id", Type::I64)], &schema()).unwrap(),
        )),
        tape: Some(Rc::new(
            InterpreterUnit::prepare_with_tools(NATIVE, &[("batch_id", Type::I64)], &schema())
                .unwrap(),
        )),
        monty: Some(monty_compile(false)),
        host: Some(monty_compile(true)),
    }
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let reps: usize = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "20".into())
        .parse()
        .unwrap();
    println!(
        "{}",
        serde_json::json!({"kind":"identity","engine":"ar-contains","reps":reps,
        "rust":env!("CARGO_PKG_VERSION"), "arch":std::env::consts::ARCH,"os":std::env::consts::OS,
        "source_native":NATIVE,"source_monty":MONTY,"source_host":HOST})
    );
    let engines = ["native", "tape", "monty", "host-bool"];
    for mode in ["cold", "warm"] {
        let prepared = prepared(mode);
        for (label, text) in [
            ("short-hit", "error near start".to_owned()),
            ("long-hit", format!("{}error", "a".repeat(16 * 1024))),
            ("long-miss", "a".repeat(16 * 1024)),
            ("escaped-hit", "\u{000a}error".repeat(512)),
            ("max-miss", "a".repeat(64 * 1024 - 2)),
        ] {
            let data =
                Bytes::from(serde_json::to_vec(&serde_json::json!({"message":text})).unwrap());
            let expected = if label == "long-miss" || label == "max-miss" {
                "False\n"
            } else {
                "True\n"
            };
            // Warmup all engines in each shape; no warmup run is reported.
            for engine in engines {
                let (_, out) = once(
                    engine,
                    mode,
                    Arc::new(P {
                        bytes: data.clone(),
                    }),
                    &prepared,
                )
                .await;
                assert_eq!(out, expected);
            }
            for i in 0..reps {
                // Balanced order rotates forward/reverse to avoid an always-first winner.
                let order = if i % 2 == 0 {
                    engines
                } else {
                    [engines[3], engines[2], engines[1], engines[0]]
                };
                for engine in order {
                    let (ns, out) = once(
                        engine,
                        mode,
                        Arc::new(P {
                            bytes: data.clone(),
                        }),
                        &prepared,
                    )
                    .await;
                    assert_eq!(out, expected);
                    println!(
                        "{}",
                        serde_json::json!({"kind":"run","mode":mode,"case":label,"engine":engine,"rep":i,"ns":ns,"bytes":data.len(),"result":out.trim()})
                    );
                }
            }
        }
    }
}

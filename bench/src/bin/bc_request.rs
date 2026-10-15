//! BC: prestarted worker, one fresh source per JSON-line request.
#[path = "../v2_common.rs"]
mod common;
use common::{BATCH, LIMIT, RequestProvider};
use minipy_native::provider::Provider;
use minipy_native::{
    codegen::CodeUnit,
    interpreter,
    interpreter::InterpreterUnit,
    ir::Type,
    runtime::{self, Cancellation, ScriptLimits},
};
use monty::{MontyRun, RunProgress};
use monty_types::{
    CompileOptions, ExtFunctionResult, MontyObject, MontyUuid, PrintWriter, ResourceTracker,
};
use serde::{Deserialize, Serialize};
use std::{
    io::{self, BufRead, Write},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

const INPUTS: &[(&str, Type)] = &[("batch_id", Type::I64), ("price_limit", Type::I64)];
#[derive(Deserialize)]
struct Request {
    source: String,
    rows: usize,
}
#[derive(Serialize)]
struct Response {
    stdout: String,
    loads: usize,
    records: usize,
}

async fn run(engine: &str, request: Request) -> Result<Response, String> {
    let mut provider = RequestProvider::ready(request.rows, Duration::ZERO);
    provider.observe = true;
    let stats = provider.stats.clone();
    let provider = Arc::new(provider);
    let limits = ScriptLimits::default();
    let stdout = match engine {
        "native" => {
            let code = Rc::new(
                CodeUnit::compile_script(&request.source, INPUTS).map_err(|e| e.to_string())?,
            );
            let out = runtime::execute_script(
                code,
                provider,
                &[BATCH, LIMIT],
                limits,
                Cancellation::default(),
                None,
            )
            .await;
            out.status.map_err(|e| e.to_string())?;
            out.stdout
        }
        "interpreter" => {
            let code = Rc::new(
                InterpreterUnit::prepare_script(&request.source, INPUTS)
                    .map_err(|e| e.to_string())?,
            );
            let out = interpreter::execute_script(
                code,
                provider,
                &[BATCH, LIMIT],
                limits,
                Cancellation::default(),
                None,
            )
            .await;
            out.status.map_err(|e| e.to_string())?;
            out.stdout
        }
        "monty" => {
            let code = MontyRun::new(
                request.source,
                "common.py",
                vec!["batch_id".into(), "price_limit".into(), "tools".into()],
                CompileOptions::default(),
            )
            .map_err(|e| e.to_string())?;
            let mut stdout = String::new();
            let mut progress = code
                .start(
                    vec![
                        MontyObject::int(BATCH),
                        MontyObject::int(LIMIT),
                        MontyObject::module(
                            "tools",
                            MontyUuid::from_u128(1),
                            ["load_quotes", "record_total"].into_iter().map(|name| {
                                (MontyObject::string(name), MontyObject::function(name, None))
                            }),
                        ),
                    ],
                    ResourceTracker::default(),
                    PrintWriter::CollectString(&mut stdout, Some(16384)),
                )
                .map_err(|e| e.to_string())?;
            loop {
                match progress {
                    RunProgress::Complete(_) => break,
                    RunProgress::FunctionCall(call) => {
                        let id = call.call_id;
                        let batch = call
                            .args
                            .arg(0)
                            .and_then(|v| v.as_int())
                            .ok_or("bad batch")?;
                        let total = call.args.arg(1).and_then(|v| v.as_int());
                        let load = call.function_name == "load_quotes";
                        if !load && call.function_name != "record_total" {
                            return Err("unknown tool".into());
                        }
                        let RunProgress::ResolveFutures(wait) = call
                            .resume_pending(PrintWriter::CollectString(&mut stdout, Some(16384)))
                            .map_err(|e| e.to_string())?
                        else {
                            return Err("no await".into());
                        };
                        let (tx, rx) = tokio::sync::oneshot::channel();
                        let p = provider.clone();
                        tokio::spawn(async move {
                            let value = if load {
                                p.load_quotes(batch).await.map(|b| {
                                    let class = MontyObject::class_type(
                                        "Quote",
                                        MontyUuid::from_u128(3),
                                        true,
                                        false,
                                        [],
                                    );
                                    MontyObject::list(b.rows().iter().enumerate().map(|(i, q)| {
                                        MontyObject::class_instance(
                                            class.clone(),
                                            MontyUuid::from_u128(i as u128 + 1000),
                                            [
                                                (
                                                    MontyObject::string("unit_price_cents"),
                                                    MontyObject::int(q.unit_price_cents),
                                                ),
                                                (
                                                    MontyObject::string("quantity"),
                                                    MontyObject::int(q.quantity),
                                                ),
                                            ],
                                        )
                                    }))
                                })
                            } else {
                                p.record_total(batch, total.expect("record total"))
                                    .await
                                    .map(MontyObject::int)
                            };
                            let _ = tx.send(value);
                        });
                        let value = rx
                            .await
                            .map_err(|e| e.to_string())?
                            .map_err(|e| e.to_string())?;
                        progress = wait
                            .resume(
                                vec![(id, ExtFunctionResult::Return(value))],
                                PrintWriter::CollectString(&mut stdout, Some(16384)),
                            )
                            .map_err(|e| e.to_string())?;
                    }
                    _ => return Err("unexpected progress".into()),
                }
            }
            stdout
        }
        _ => return Err("unknown engine".into()),
    };
    use std::sync::atomic::Ordering;
    Ok(Response {
        stdout,
        loads: stats.loads.load(Ordering::Acquire),
        records: stats.records.load(Ordering::Acquire),
    })
}
fn main() {
    let engine = std::env::args().nth(1).expect("engine");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "{{\"ready\":true}}").unwrap();
    stdout.flush().unwrap();
    for line in stdin.lock().lines() {
        let result = line.map_err(|e| e.to_string()).and_then(|line| {
            let req: Request = serde_json::from_str(&line).map_err(|e| e.to_string())?;
            rt.block_on(run(&engine, req))
        });
        let reply = match result {
            Ok(v) => serde_json::json!({"ok":v}),
            Err(e) => serde_json::json!({"error":e}),
        };
        writeln!(stdout, "{reply}").unwrap();
        stdout.flush().unwrap();
    }
}

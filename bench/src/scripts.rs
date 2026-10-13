//! Bounded D comparison: real scripts, one-shot compilation and ordinary Monty.
//! argv: native|monty scalar|scan|weighted|legacy compile|cold|warm|mixed ROWS REPS DELAY_US
mod script_memory;
mod v2_common;
mod weak_memory;
use minipy_native::{
    codegen::CodeUnit,
    ir::Type,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
};
use monty::{MontyRun, RunProgress};
use monty_types::{CompileOptions, ExtFunctionResult, MontyObject, PrintWriter, ResourceTracker};
use std::{
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use v2_common::{BATCH, LIMIT, RequestProvider};
#[cfg(feature = "allocation-diagnostics")]
#[global_allocator]
static ALLOC: v2_common::CountingAllocator = v2_common::CountingAllocator;
fn source(case: &str) -> String {
    if case == "legacy" {
        return v2_common::source("weighted_scan", 0).unwrap();
    }
    let file = match case {
        "scalar" => "scalar_script.py",
        "scan" => "scan_script.py",
        "weighted" => "weighted_script.py",
        "print_loop" => "print_loop_script.py",
        _ => panic!("case"),
    };
    let base = std::env::var_os("MINIPY_FIXTURES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures"));
    std::fs::read_to_string(base.join(file)).unwrap()
}
fn monty(source: &str, legacy: bool) -> MontyRun {
    let source = if legacy {
        v2_common::monty_source(source)
    } else {
        source
            .replace("tools.load_quotes(", "await load_quotes(")
            .replace("tools.record_total(", "await record_total(")
            .replace("quote.unit_price_cents", "quote[0]")
            .replace("quote.quantity", "quote[1]")
    };
    MontyRun::new(
        source,
        "script.py",
        vec![
            "batch_id".into(),
            "price_limit".into(),
            "load_quotes".into(),
            "record_total".into(),
        ],
        CompileOptions::default(),
    )
    .unwrap()
}
async fn monty_run(
    code: MontyRun,
    p: Arc<RequestProvider>,
) -> Result<(String, Option<i64>), String> {
    let mut stdout = String::new();
    let mut progress = code
        .start(
            vec![
                MontyObject::int(BATCH),
                MontyObject::int(LIMIT),
                MontyObject::function("load_quotes", None),
                MontyObject::function("record_total", None),
            ],
            ResourceTracker::default(),
            PrintWriter::CollectString(&mut stdout, Some(16 * 1024)),
        )
        .map_err(|e| e.to_string())?;
    loop {
        match progress {
            RunProgress::Complete(v) => return Ok((stdout, v.as_ref().as_int())),
            RunProgress::FunctionCall(call) => {
                let batch = call.args.arg(0).and_then(|x| x.as_int()).ok_or("batch")?;
                let total = call.args.arg(1).and_then(|x| x.as_int());
                let load = call.function_name == "load_quotes";
                if !load && call.function_name != "record_total" {
                    return Err("unexpected tool".into());
                }
                let id = call.call_id;
                let RunProgress::ResolveFutures(wait) = call
                    .resume_pending(PrintWriter::CollectString(&mut stdout, Some(16 * 1024)))
                    .map_err(|e| e.to_string())?
                else {
                    return Err("expected real await".into());
                };
                let (tx, rx) = tokio::sync::oneshot::channel();
                let p = p.clone();
                tokio::spawn(async move {
                    let result = if load {
                        p.load_quotes(batch).await.map(|b| v2_common::object(&b))
                    } else {
                        p.record_total(batch, total.unwrap())
                            .await
                            .map(MontyObject::int)
                    };
                    let _ = tx.send(result);
                });
                let value = rx
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())?;
                progress = wait
                    .resume(
                        vec![(id, ExtFunctionResult::Return(value))],
                        PrintWriter::CollectString(&mut stdout, Some(16 * 1024)),
                    )
                    .map_err(|e| e.to_string())?;
            }
            _ => return Err("unexpected progress".into()),
        }
    }
}
async fn native_run(
    code: Rc<CodeUnit>,
    p: Arc<RequestProvider>,
    legacy: bool,
) -> Result<(String, Option<i64>), String> {
    let mut options = ScriptLimits::default();
    options.limits.rows = 10_000_000;
    options.limits.buffer_bytes = 200_000_000;
    if legacy {
        runtime::execute(
            code,
            p,
            BATCH,
            LIMIT,
            options.limits,
            Cancellation::default(),
            None,
        )
        .await
        .map(|r| (String::new(), Some(r)))
        .map_err(|e| e.to_string())
    } else {
        let out = runtime::execute_script(
            code,
            p,
            &[BATCH, LIMIT],
            options,
            Cancellation::default(),
            None,
        )
        .await;
        out.status
            .map(|_| (out.stdout, None))
            .map_err(|e| e.to_string())
    }
}
fn expected(case: &str, n: usize) -> (String, Option<i64>) {
    let expected = v2_common::oracle(n, "weighted_scan", 0);
    if case == "legacy" {
        (String::new(), Some(expected))
    } else {
        let stdout = match case {
            "scalar" => "734\nTrue\n".into(),
            "scan" => format!("{}\n", expected - BATCH),
            "weighted" => format!("{expected}\n"),
            "print_loop" => v2_common::rows(n)
                .iter()
                .map(|q| format!("{}\n", q.quantity))
                .collect(),
            _ => unreachable!(),
        };
        (stdout, None)
    }
}
fn validate(result: &(String, Option<i64>), expected: &(String, Option<i64>), legacy: bool) {
    assert_eq!(result.0, expected.0);
    if legacy {
        assert_eq!(result.1, expected.1);
    }
}
fn native(source: &str) -> Rc<CodeUnit> {
    Rc::new(
        CodeUnit::compile_script(
            source,
            &[("batch_id", Type::I64), ("price_limit", Type::I64)],
        )
        .unwrap(),
    )
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 7, "engine case mode rows reps delay_us");
    let engine = &args[1];
    assert!(["native", "monty"].contains(&engine.as_str()));
    let case = &args[2];
    let mode = &args[3];
    let n: usize = args[4].parse().unwrap();
    let reps: usize = args[5].parse().unwrap();
    let delay = Duration::from_micros(args[6].parse().unwrap());
    assert!(reps > 0 && n <= 1_000_000);
    assert!(["compile", "cold", "warm", "mixed", "hold", "weak"].contains(&mode.as_str()));
    let source = source(case);
    let legacy = case == "legacy";
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    if mode == "weak" {
        assert!(engine == "native" && case == "weighted");
        if !cfg!(feature = "allocation-diagnostics") {
            panic!("weak is a separate diagnostic build");
        }
        tokio::task::LocalSet::new().block_on(&rt, weak_memory::run(&source, n));
        return;
    }
    if mode == "hold" {
        assert!(["weighted", "legacy"].contains(&case.as_str()));
        tokio::task::LocalSet::new().block_on(
            &rt,
            script_memory::run(engine, &source, case, n, reps, delay),
        );
        return;
    }
    if mode == "mixed" {
        let expected_short = Rc::new(expected(case, 100));
        let expected_long = Rc::new(expected(case, n));
        let local = tokio::task::LocalSet::new();
        let cpu = v2_common::cpu_ns();
        let start = Instant::now();
        let observations = local.block_on(&rt, async {
            let mut jobs = Vec::new();
            for i in 0..reps {
                let engine = engine.clone();
                let source = source.clone();
                let expected = if i % 4 != 0 {
                    expected_short.clone()
                } else {
                    expected_long.clone()
                };
                jobs.push(tokio::task::spawn_local(async move {
                    let short = i % 4 != 0;
                    let n = if short { 100 } else { n };
                    let start = Instant::now();
                    let p = Arc::new(RequestProvider::ready(n, delay));
                    let result = if engine == "native" {
                        native_run(native(&source), p, legacy).await
                    } else {
                        monty_run(monty(&source, legacy), p).await
                    }
                    .unwrap();
                    let wall = start.elapsed().as_nanos() as u64;
                    validate(&result, &expected, legacy);
                    (short, wall)
                }));
            }
            let mut out = Vec::new();
            for j in jobs {
                out.push(j.await.unwrap());
            }
            out
        });
        v2_common::emit(
            &serde_json::json!({"engine":engine,"case":case,"mode":mode,"rows_long":n,"reps":reps,"delay_us":delay.as_micros(),"total_wall_ns":start.elapsed().as_nanos() as u64,"total_cpu_ns":v2_common::cpu_ns()-cpu,"observations":observations,"completed":observations.len()}),
        );
        return;
    }
    let prepared_native = if mode == "warm" && engine == "native" {
        Some(native(&source))
    } else {
        None
    };
    let prepared_monty = if mode == "warm" && engine == "monty" {
        Some(monty(&source, legacy))
    } else {
        None
    };
    let expected = expected(case, n);
    for i in 0..reps {
        if cfg!(feature = "allocation-diagnostics") {
            v2_common::reset_peak();
        }
        let before = v2_common::allocation();
        let cpu = v2_common::cpu_ns();
        let whole = Instant::now();
        // Clone and fresh compile are in the cold interval; no source/IR cache.
        let request_source = if mode == "warm" {
            None
        } else {
            Some(source.clone())
        };
        let request_text = request_source.as_deref().unwrap_or(&source);
        let build = Instant::now();
        let code = if engine == "native" {
            Some(
                prepared_native
                    .clone()
                    .unwrap_or_else(|| native(request_text)),
            )
        } else {
            None
        };
        let m = if engine == "monty" {
            Some(
                prepared_monty
                    .clone()
                    .unwrap_or_else(|| monty(request_text, legacy)),
            )
        } else {
            None
        };
        let compile_ns = if mode == "warm" {
            0
        } else {
            build.elapsed().as_nanos() as u64
        };
        let code_bytes = code.as_ref().map(|c| c.code_bytes);
        let result = if mode == "compile" {
            drop(code);
            drop(m);
            None
        } else {
            let p = Arc::new(RequestProvider::ready(n, delay));
            Some(
                rt.block_on(async {
                    if let Some(code) = code {
                        native_run(code, p, legacy).await
                    } else {
                        monty_run(m.unwrap(), p).await
                    }
                })
                .unwrap(),
            )
        };
        drop(request_source);
        let total_wall_ns = whole.elapsed().as_nanos() as u64;
        let total_cpu_ns = v2_common::cpu_ns() - cpu;
        let after = v2_common::allocation();
        // Return/output buffers deliberately stay live until correctness checking.
        // Compiler, Task, provider operations, buffer import and native cleanup are timed.
        if let Some(result) = result.as_ref() {
            validate(result, &expected, legacy);
        }
        v2_common::emit(
            &serde_json::json!({"engine":engine,"case":case,"mode":mode,"rows":n,"rep":i,"delay_us":delay.as_micros(),"compile_ns":compile_ns,"total_wall_ns":total_wall_ns,"total_cpu_ns":total_cpu_ns,"code_bytes":code_bytes,"allocation_delta":v2_common::allocation_delta(before,after),"completed":true,"stdout":result.as_ref().filter(|r|r.0.len()<=128).map(|r|&r.0),"stdout_bytes":result.as_ref().map(|r|r.0.len())}),
        );
    }
}

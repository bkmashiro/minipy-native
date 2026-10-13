use minipy_native::{
    buffer::{Quote, QuoteBuffer},
    codegen::CodeUnit,
    provider::Provider,
    runtime::{self, Cancellation, Limits, Trace},
};
use monty::{MontyRun, RunProgress};
use monty_types::{CompileOptions, MontyObject, PrintWriter, ResourceTracker};
use std::{
    future::Future,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const ROW_COUNTS: [usize; 3] = [0, 100, 10_000];
const ITERS: usize = 20;
const BATCH: i64 = 17;
const LIMIT: i64 = 700;

struct ImmediateProvider {
    source: Mutex<Option<QuoteBuffer>>,
}
impl Provider for ImmediateProvider {
    fn load_quotes(
        &self,
        _batch: i64,
    ) -> impl Future<Output = Result<QuoteBuffer, minipy_native::error::TaskError>> + Send {
        let rows = self.source.lock().unwrap().take().ok_or_else(|| {
            minipy_native::error::TaskError::Provider("one-shot provider already consumed".into())
        });
        async move { rows }
    }
    fn record_total(
        &self,
        batch: i64,
        total: i64,
    ) -> impl Future<Output = Result<i64, minipy_native::error::TaskError>> + Send {
        async move {
            total
                .checked_add(batch)
                .ok_or(minipy_native::error::TaskError::Overflow)
        }
    }
}

fn input(n: usize) -> Vec<Quote> {
    (0..n)
        .map(|i| Quote {
            unit_price_cents: ((i * 37) % 1000) as i64,
            quantity: ((i % 9) as i64) - 2,
        })
        .collect()
}
fn expected(rows: &[Quote], stateful: bool) -> i64 {
    let mut total = 0i64;
    let mut prev = 0i64;
    for q in rows {
        if q.quantity > 0
            && q.unit_price_cents <= LIMIT
            && (!stateful || q.unit_price_cents >= prev)
        {
            total += q.unit_price_cents * q.quantity;
            if stateful {
                prev = q.unit_price_cents;
            }
        }
    }
    total + BATCH
}
fn monty_input(rows: &[Quote]) -> (Vec<MontyObject>, MontyObject, Duration) {
    let start = Instant::now();
    let values: Vec<_> = rows
        .iter()
        .map(|q| {
            MontyObject::tuple(vec![
                MontyObject::int(q.unit_price_cents),
                MontyObject::int(q.quantity),
            ])
        })
        .collect();
    let quotes = MontyObject::list(values);
    (
        vec![
            MontyObject::int(BATCH),
            MontyObject::int(LIMIT),
            MontyObject::function("load_quotes", None),
            MontyObject::function("record_total", None),
        ],
        quotes,
        start.elapsed(),
    )
}
fn run_monty(
    runner: MontyRun,
    args: Vec<MontyObject>,
    quotes: MontyObject,
) -> Result<(i64, u8), String> {
    let progress = runner
        .start(args, ResourceTracker::default(), PrintWriter::Disabled)
        .map_err(|e| e.to_string())?;
    let progress = match progress {
        RunProgress::FunctionCall(call) if call.function_name == "load_quotes" => call
            .resume(quotes, PrintWriter::Disabled)
            .map_err(|e| e.to_string())?,
        x => return Err(format!("expected load_quotes call, got {x:?}")),
    };
    let progress = match progress {
        RunProgress::FunctionCall(call) if call.function_name == "record_total" => {
            let total = call
                .args
                .arg(1)
                .ok_or("missing total")?
                .as_int()
                .ok_or("total not int")?;
            call.resume(MontyObject::int(total + BATCH), PrintWriter::Disabled)
                .map_err(|e| e.to_string())?
        }
        x => return Err(format!("expected record_total call, got {x:?}")),
    };
    match progress {
        RunProgress::Complete(v) => Ok((v.as_ref().as_int().ok_or("receipt not int")?, 2)),
        x => Err(format!("expected completion, got {x:?}")),
    }
}

fn monty_script(stateful: bool) -> String {
    if stateful {
        "quotes = load_quotes(batch_id)\ntotal = 0\nprevious_price = 0\ndescending = False\nfor q in quotes:\n    if q[1] > 0 and q[0] <= price_limit and q[0] >= previous_price:\n        total += q[0] * q[1]\n        previous_price = q[0]\n    else:\n        descending = True\n        total -= 0\nif descending:\n    total += 0\nrecord_total(batch_id, total)\n".into()
    } else {
        "quotes = load_quotes(batch_id)\ntotal = 0\nfor q in quotes:\n    if q[1] > 0 and q[0] <= price_limit:\n        total += q[0] * q[1]\nrecord_total(batch_id, total)\n".into()
    }
}
fn runner(stateful: bool) -> Result<MontyRun, String> {
    MontyRun::new(
        monty_script(stateful),
        "quote.py",
        vec![
            "batch_id".into(),
            "price_limit".into(),
            "load_quotes".into(),
            "record_total".into(),
        ],
        CompileOptions::default(),
    )
    .map_err(|e| e.to_string())
}

// Fixed host helper baseline: ordinary checked scan, not the native execution path.
fn host_helper(
    rows: &[Quote],
    price_limit: i64,
    batch: i64,
    stateful: bool,
) -> Result<i64, minipy_native::error::TaskError> {
    let mut total = 0i64;
    let mut previous = 0i64;
    for q in rows {
        if q.quantity > 0
            && q.unit_price_cents <= price_limit
            && (!stateful || q.unit_price_cents >= previous)
        {
            total = total
                .checked_add(
                    q.unit_price_cents
                        .checked_mul(q.quantity)
                        .ok_or(minipy_native::error::TaskError::Overflow)?,
                )
                .ok_or(minipy_native::error::TaskError::Overflow)?;
            if stateful {
                previous = q.unit_price_cents;
            }
        }
    }
    total
        .checked_add(batch)
        .ok_or(minipy_native::error::TaskError::Overflow)
}

fn helper_runner() -> Result<MontyRun, String> {
    MontyRun::new("handle = load_quotes(batch_id)\ntotal = aggregate_quotes(handle, price_limit)\nrecord_total(batch_id, total)\n".into(),
        "helper.py", vec!["batch_id".into(), "price_limit".into(), "load_quotes".into(),
            "aggregate_quotes".into(), "record_total".into()], CompileOptions::default()).map_err(|e| e.to_string())
}
fn run_monty_helper(runner: MontyRun, n: usize, stateful: bool) -> Result<i64, String> {
    let args = vec![
        MontyObject::int(BATCH),
        MontyObject::int(LIMIT),
        MontyObject::function("load_quotes", None),
        MontyObject::function("aggregate_quotes", None),
        MontyObject::function("record_total", None),
    ];
    let p = runner
        .start(args, ResourceTracker::default(), PrintWriter::Disabled)
        .map_err(|e| e.to_string())?;
    let RunProgress::FunctionCall(load) = p else {
        return Err("expected helper load".into());
    };
    assert_eq!(load.function_name, "load_quotes");
    // Exact same immutable QuoteBuffer data layer as native. Monty receives an opaque handle,
    // not a list of objects and not a per-field callback. This helper is a finite host capability.
    let buffer = QuoteBuffer::from_quotes(input(n)).map_err(|e| e.to_string())?;
    let p = load
        .resume(MontyObject::int(1), PrintWriter::Disabled)
        .map_err(|e| e.to_string())?;
    let RunProgress::FunctionCall(aggregate) = p else {
        return Err("expected aggregate".into());
    };
    assert_eq!(aggregate.function_name, "aggregate_quotes");
    assert_eq!(aggregate.args.arg(0).unwrap().as_int(), Some(1));
    assert_eq!(aggregate.args.arg(1).unwrap().as_int(), Some(LIMIT));
    let receipt = host_helper(buffer.rows(), LIMIT, BATCH, stateful).map_err(|e| e.to_string())?;
    drop(buffer);
    let p = aggregate
        .resume(MontyObject::int(receipt - BATCH), PrintWriter::Disabled)
        .map_err(|e| e.to_string())?;
    let RunProgress::FunctionCall(record) = p else {
        return Err("expected helper record".into());
    };
    assert_eq!(record.function_name, "record_total");
    assert_eq!(record.args.arg(1).unwrap().as_int(), Some(receipt - BATCH));
    let p = record
        .resume(MontyObject::int(receipt), PrintWriter::Disabled)
        .map_err(|e| e.to_string())?;
    let RunProgress::Complete(v) = p else {
        return Err("expected helper completion".into());
    };
    v.as_ref()
        .as_int()
        .ok_or_else(|| "helper receipt not int".into())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opt = std::env::args().nth(1).unwrap_or_else(|| "speed".into());
    let rt = tokio::runtime::Builder::new_current_thread().build()?;
    println!("# monty=56949a4c45f51cedc7a96450782f58a8017e114d,provider=ready,opt={opt}");
    println!(
        "case,rows,engine,mode,chunk,sample,compile_ns,conversion_ns,execute_cleanup_ns,total_ns,code_bytes,receipt"
    );
    for (case, file, stateful) in [
        ("weighted_scan", "weighted_scan.py", false),
        ("stateful_branch", "stateful_branch.py", true),
    ] {
        let fixtures = std::env::var_os("MINIPY_FIXTURES")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures")
            });
        let native_source = std::fs::read_to_string(fixtures.join(file))?;
        let native_warm = Rc::new(CodeUnit::compile_with_opt(&native_source, &opt)?);
        let dummy = Arc::new(ImmediateProvider {
            source: Mutex::new(Some(QuoteBuffer::from_quotes(vec![])?)),
        });
        let load = dummy.load_quotes(BATCH);
        let record = dummy.record_total(BATCH, 0);
        let future = runtime::execute(
            native_warm.clone(),
            dummy.clone(),
            BATCH,
            LIMIT,
            Limits::default(),
            Cancellation::default(),
            None,
        );
        eprintln!(
            "layout,{case},task_future={},code_unit={},provider={},buffer_handle={},cancellation_handle={},load_future={},record_future={}",
            std::mem::size_of_val(&future),
            std::mem::size_of::<CodeUnit>(),
            std::mem::size_of::<ImmediateProvider>(),
            std::mem::size_of::<QuoteBuffer>(),
            std::mem::size_of::<Cancellation>(),
            std::mem::size_of_val(&load),
            std::mem::size_of_val(&record)
        );
        drop(future);
        drop(load);
        drop(record);
        drop(dummy);
        let monty_warm = runner(stateful)?;
        let helper_warm = helper_runner()?;
        for n in ROW_COUNTS {
            let rows = input(n);
            let receipt = expected(&rows, stateful);
            // This provider is only for the separate untimed retention observation.
            let provider = Arc::new(ImmediateProvider {
                source: Mutex::new(Some(QuoteBuffer::from_quotes(rows.clone())?)),
            });
            for mode in ["cold", "warm"] {
                for i in 0..ITERS {
                    let start = Instant::now();
                    let t = Instant::now();
                    let mr = if mode == "cold" {
                        runner(stateful)?
                    } else {
                        monty_warm.clone()
                    };
                    let compile = t.elapsed().as_nanos(); // Warm includes required runner clone.
                    let t = Instant::now();
                    let produced = input(n);
                    let (args, quotes, _) = monty_input(&produced);
                    drop(produced);
                    let conversion = t.elapsed();
                    let t = Instant::now();
                    let (r, calls) = run_monty(mr, args, quotes).map_err(std::io::Error::other)?;
                    let execution = t.elapsed().as_nanos();
                    let total = start.elapsed().as_nanos();
                    assert_eq!((r, calls), (receipt, 2));
                    println!(
                        "{case},{n},monty,{mode},0,{i},{compile},{},{execution},{total},0,{r}",
                        conversion.as_nanos()
                    );
                }
                for i in 0..ITERS {
                    let start = Instant::now();
                    let t = Instant::now();
                    let runner = if mode == "cold" {
                        helper_runner()?
                    } else {
                        helper_warm.clone()
                    };
                    let compile = t.elapsed().as_nanos();
                    let t = Instant::now();
                    let r = run_monty_helper(runner, n, stateful).map_err(std::io::Error::other)?;
                    let execution = t.elapsed().as_nanos();
                    let total = start.elapsed().as_nanos();
                    assert_eq!(r, receipt);
                    println!(
                        "{case},{n},monty_helper,{mode},0,{i},{compile},0,{execution},{total},0,{r}"
                    );
                }
                for chunk in [1, 256, 4096] {
                    for i in 0..ITERS {
                        let start = Instant::now();
                        let t = Instant::now();
                        let code = if mode == "cold" {
                            Rc::new(CodeUnit::compile_with_opt(&native_source, &opt)?)
                        } else {
                            native_warm.clone()
                        };
                        let compile = t.elapsed().as_nanos();
                        let bytes = code.code_bytes;
                        let t = Instant::now();
                        let request_provider = Arc::new(ImmediateProvider {
                            source: Mutex::new(Some(QuoteBuffer::from_quotes(input(n))?)),
                        });
                        let conversion = t.elapsed().as_nanos();
                        let t = Instant::now();
                        let result = rt.block_on(runtime::execute(
                            code,
                            request_provider,
                            BATCH,
                            LIMIT,
                            Limits {
                                chunk,
                                ..Limits::default()
                            },
                            Cancellation::default(),
                            None,
                        ))?;
                        let execution = t.elapsed().as_nanos();
                        let total = start.elapsed().as_nanos();
                        assert_eq!(result, receipt);
                        // Production and buffer import are in conversion; ownership transfer is in execute.
                        println!(
                            "{case},{n},native,{mode},{chunk},{i},{compile},{conversion},{execution},{total},{bytes},{result}"
                        );
                    }
                }
            }
            for i in 0..ITERS {
                let start = Instant::now();
                let t = Instant::now();
                let buffer = QuoteBuffer::from_quotes(input(n))?;
                let conversion = t.elapsed().as_nanos();
                let t = Instant::now();
                let r = host_helper(std::hint::black_box(buffer.rows()), LIMIT, BATCH, stateful)?;
                drop(buffer);
                let execution = t.elapsed().as_nanos();
                let total = start.elapsed().as_nanos();
                assert_eq!(r, receipt);
                println!("{case},{n},host,warm,0,{i},0,{conversion},{execution},{total},0,{r}");
            }
            // Untimed lifecycle observation; production measurements above have no trace allocation.
            let trace: Trace = Rc::default();
            rt.block_on(runtime::execute(
                native_warm.clone(),
                provider.clone(),
                BATCH,
                LIMIT,
                Limits::default(),
                Cancellation::default(),
                Some(trace.clone()),
            ))?;
            for e in trace.borrow().iter().filter(|e| e.phase != "yield") {
                eprintln!(
                    "resource,{case},{n},{},{},{},{},{}",
                    e.phase,
                    e.buffer_bytes,
                    e.frame_bytes,
                    native_warm.code_bytes,
                    native_warm.clif.len()
                );
            }
        }
        // Ready-buffer control: isolate scan/scheduling fairness; producer cost is not in mixed latency.
        for chunk in [1, 256, 4096] {
            let heavy = Arc::new(ImmediateProvider {
                source: Mutex::new(Some(QuoteBuffer::from_quotes(input(1_000_000))?)),
            });
            let light = QuoteBuffer::from_quotes(input(100))?;
            let local = tokio::task::LocalSet::new();
            let code = native_warm.clone();
            let heavy_receipt = expected(&input(1_000_000), stateful);
            rt.block_on(local.run_until(async move {
                let heavy_start = Instant::now();
                let heavy_code = code.clone();
                let h = tokio::task::spawn_local(async move {
                    let r = runtime::execute(
                        heavy_code,
                        heavy,
                        BATCH,
                        LIMIT,
                        Limits {
                            chunk,
                            ..Limits::default()
                        },
                        Cancellation::default(),
                        None,
                    )
                    .await
                    .unwrap();
                    let ns = heavy_start.elapsed().as_nanos();
                    assert_eq!(r, heavy_receipt);
                    ns
                });
                let mut tasks = Vec::new();
                for _ in 0..20 {
                    let start = Instant::now();
                    let c = code.clone();
                    let p = Arc::new(ImmediateProvider {
                        source: Mutex::new(Some(light.clone())),
                    });
                    tasks.push(tokio::task::spawn_local(async move {
                        let r = runtime::execute(
                            c,
                            p,
                            BATCH,
                            LIMIT,
                            Limits {
                                chunk,
                                ..Limits::default()
                            },
                            Cancellation::default(),
                            None,
                        )
                        .await
                        .unwrap();
                        let ns = start.elapsed().as_nanos();
                        assert_eq!(r, expected(&input(100), stateful));
                        ns
                    }));
                }
                for (i, task) in tasks.into_iter().enumerate() {
                    let ns = task.await.unwrap();
                    println!("{case},100,native,mixed_light,{chunk},{i},0,0,{ns},{ns},0,0");
                }
                let ns = h.await.unwrap();
                println!("{case},1000000,native,mixed_heavy,{chunk},0,0,0,{ns},{ns},0,0");
            }));
            for i in 0..8 {
                let p = Arc::new(ImmediateProvider {
                    source: Mutex::new(Some(QuoteBuffer::from_quotes(input(1_000_000))?)),
                });
                let trace: Trace = Rc::default();
                let cancel = Cancellation::default();
                let requested = Rc::new(std::cell::Cell::new(None::<Instant>));
                let task = runtime::execute(
                    native_warm.clone(),
                    p.clone(),
                    BATCH,
                    LIMIT,
                    Limits {
                        chunk,
                        ..Limits::default()
                    },
                    cancel.clone(),
                    Some(trace.clone()),
                );
                let canceller = async {
                    loop {
                        if trace.borrow().iter().any(|e| e.phase == "yield") {
                            requested.set(Some(Instant::now()));
                            cancel.cancel();
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                };
                let ns = rt.block_on(async {
                    let (result, ()) = tokio::join!(task, canceller);
                    assert_eq!(result, Err(minipy_native::error::TaskError::Cancelled));
                    requested.get().unwrap().elapsed().as_nanos()
                });
                assert!(p.source.lock().unwrap().is_none());
                println!("{case},1000000,native,cancellation,{chunk},{i},0,0,{ns},{ns},0,0");
            }
        }
    }
    Ok(())
}

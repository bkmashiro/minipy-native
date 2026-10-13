mod v2_common;
use minipy_native::{
    buffer::QuoteBuffer,
    codegen::CodeUnit,
    error::TaskError,
    provider::Provider,
    runtime::{self, Cancellation, Limits},
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use v2_common::*;
#[global_allocator]
#[cfg(feature = "allocation-diagnostics")]
static ALLOCATOR: CountingAllocator = CountingAllocator;

struct Prepared {
    buffer: Mutex<Option<QuoteBuffer>>,
    delay: Duration,
}
struct MontyPrepared {
    value: Mutex<Option<monty_types::MontyObject>>,
    delay: Duration,
}
async fn prepared_monty(code: monty::MontyRun, p: Arc<MontyPrepared>) -> Result<i64, String> {
    let (id, wait) = pending(start_monty(code)?, "load_quotes")?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    let provider = p.clone();
    tokio::spawn(async move {
        let value = provider.value.lock().unwrap().take().unwrap();
        if !provider.delay.is_zero() {
            tokio::time::sleep(provider.delay).await;
        }
        let _ = tx.send(value);
    });
    let progress = resolve(id, wait, rx.await.map_err(|e| e.to_string())?)?;
    let total = record_total(&progress)?
        .checked_add(BATCH)
        .ok_or("overflow")?;
    let (id, wait) = pending(progress, "record_total")?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        if !p.delay.is_zero() {
            tokio::time::sleep(p.delay).await;
        }
        let _ = tx.send(monty_types::MontyObject::int(total));
    });
    receipt(resolve(id, wait, rx.await.map_err(|e| e.to_string())?)?)
}
impl Provider for Prepared {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        let b = self
            .buffer
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| TaskError::Provider("already consumed".into()))?;
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        Ok(b)
    }
    async fn record_total(&self, batch: i64, total: i64) -> Result<i64, TaskError> {
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        total.checked_add(batch).ok_or(TaskError::Overflow)
    }
}
#[derive(Serialize, Default)]
struct Phases {
    source_ns: u64,
    compile_or_clone_ns: u64,
    produce_import_ns: u64,
    dispatch_scan_wait_cleanup_ns: u64,
}
#[derive(Serialize)]
struct Timing {
    kind: &'static str,
    engine: String,
    case: String,
    rows: usize,
    lifecycle: String,
    sample: usize,
    chunk: usize,
    delay_us: u64,
    total_wall_ns: u64,
    total_cpu_ns: u64,
    phases: Phases,
    code_bytes: Option<usize>,
    receipt: i64,
    status: &'static str,
    allocation_diagnostics: Option<serde_json::Value>,
}
struct Options {
    mode: String,
    engines: Vec<String>,
    case: String,
    rows: usize,
    repeats: usize,
    chunk: usize,
    delay: Duration,
    concurrency: usize,
    requests: usize,
    sample: usize,
    mixed: bool,
}
#[derive(Clone, Copy)]
struct TaskOptions {
    chunk: usize,
    delay: Duration,
    concurrency: usize,
    sample: usize,
    mixed: bool,
}
impl Options {
    fn parse() -> Result<Self, String> {
        let mut args = std::env::args().skip(1);
        let mode = args.next().unwrap_or("latency".into());
        if !["latency", "concurrent"].contains(&mode.as_str()) {
            return Err("mode: latency|concurrent".into());
        }
        let mut values = BTreeMap::new();
        while let Some(k) = args.next() {
            let v = args.next().ok_or("expected --key value")?;
            if !k.starts_with("--") || values.insert(k, v).is_some() {
                return Err("invalid/duplicate option".into());
            }
        }
        let get = |k: &str, d: &str| values.get(k).cloned().unwrap_or(d.into());
        let num = |k: &str, d: &str| get(k, d).parse::<usize>().map_err(|e| e.to_string());
        for k in values.keys() {
            if ![
                "--engine",
                "--case",
                "--rows",
                "--repeats",
                "--chunk",
                "--delay-us",
                "--concurrency",
                "--requests",
                "--sample",
                "--mixed",
            ]
            .contains(&k.as_str())
            {
                return Err(format!("unknown option {k}"));
            }
        }
        let engines: Vec<String> = match get("--engine", "both").as_str() {
            "both" => vec!["native".into(), "monty".into()],
            "native" => vec!["native".into()],
            "monty" => vec!["monty".into()],
            _ => {
                return Err(
                    "engine: native|monty|both; helpers remain historical reference only".into(),
                );
            }
        };
        let o = Self {
            mode,
            engines,
            case: get("--case", "weighted_scan"),
            rows: num("--rows", "100")?,
            repeats: num("--repeats", "3")?,
            chunk: num("--chunk", "256")?,
            delay: Duration::from_micros(num("--delay-us", "0")? as u64),
            concurrency: num("--concurrency", "4")?,
            requests: num("--requests", "16")?,
            sample: num("--sample", "0")?,
            mixed: match get("--mixed", "false").as_str() {
                "true" => true,
                "false" => false,
                _ => return Err("mixed: true|false".into()),
            },
        };
        if o.rows > 1_000_000
            || o.chunk == 0
            || o.chunk > 4096
            || o.concurrency == 0
            || o.concurrency > 4096
            || o.requests == 0
            || o.requests > 100_000
            || o.repeats == 0
            || o.repeats > 1000
            || o.delay > Duration::from_secs(1)
        {
            return Err("option outside bounded campaign limits".into());
        }
        source(&o.case, 0)?;
        Ok(o)
    }
}
fn latency(o: &Options, rt: &tokio::runtime::Runtime) -> Result<(), String> {
    let source = source(&o.case, 0)?;
    let native = if o.engines.iter().any(|e| e == "native") {
        Some(Rc::new(
            CodeUnit::compile(&source).map_err(|e| e.to_string())?,
        ))
    } else {
        None
    };
    let monty = if o.engines.iter().any(|e| e == "monty") {
        Some(compile_monty(&source)?)
    } else {
        None
    };
    let expected = oracle(o.rows, &o.case, 0);
    for lifecycle in ["cold_source_to_result", "prepared_request"] {
        for sample in 0..=o.repeats {
            let mut engines = o.engines.clone();
            if (sample + o.sample) % 2 == 1 {
                engines.reverse();
            }
            for engine in engines {
                // One complete warmup per engine/lifecycle, not emitted as a timed sample.
                reset_peak();
                let before = allocation();
                let cpu = cpu_ns();
                let start = Instant::now();
                let t = Instant::now();
                let request_source = if lifecycle == "cold_source_to_result" {
                    source_fn(&o.case)?
                } else {
                    String::new()
                };
                let mut phases = Phases {
                    source_ns: now_ns(t),
                    ..Default::default()
                };
                let t = Instant::now();
                let (r, bytes) = if engine == "native" {
                    let code = if lifecycle == "cold_source_to_result" {
                        Rc::new(CodeUnit::compile(&request_source).map_err(|e| e.to_string())?)
                    } else {
                        native.as_ref().unwrap().clone()
                    };
                    phases.compile_or_clone_ns = now_ns(t);
                    let bytes = code.code_bytes;
                    let t = Instant::now();
                    let p = Arc::new(Prepared {
                        buffer: Mutex::new(Some(
                            QuoteBuffer::from_quotes(rows(o.rows)).map_err(|e| e.to_string())?,
                        )),
                        delay: o.delay,
                    });
                    phases.produce_import_ns = now_ns(t);
                    let t = Instant::now();
                    let r = rt
                        .block_on(runtime::execute(
                            code,
                            p,
                            BATCH,
                            LIMIT,
                            Limits {
                                chunk: o.chunk,
                                ..Default::default()
                            },
                            Cancellation::default(),
                            None,
                        ))
                        .map_err(|e| e.to_string())?;
                    phases.dispatch_scan_wait_cleanup_ns = now_ns(t);
                    (r, Some(bytes))
                } else {
                    let code = if lifecycle == "cold_source_to_result" {
                        compile_monty(&request_source)?
                    } else {
                        monty.as_ref().unwrap().clone()
                    };
                    phases.compile_or_clone_ns = now_ns(t);
                    let t = Instant::now();
                    let buffer =
                        QuoteBuffer::from_quotes(rows(o.rows)).map_err(|e| e.to_string())?;
                    let value = object(&buffer);
                    drop(buffer);
                    let p = Arc::new(MontyPrepared {
                        value: Mutex::new(Some(value)),
                        delay: o.delay,
                    });
                    phases.produce_import_ns = now_ns(t);
                    let t = Instant::now();
                    let r = rt.block_on(prepared_monty(code, p))?;
                    phases.dispatch_scan_wait_cleanup_ns = now_ns(t);
                    (r, None)
                };
                drop(request_source);
                let wall = now_ns(start);
                let cpu = cpu_ns() - cpu;
                let after = allocation();
                if r != expected {
                    return Err(format!("receipt mismatch {r} != {expected}"));
                }
                if sample > 0 {
                    emit(&Timing {
                        kind: "latency",
                        engine,
                        case: o.case.clone(),
                        rows: o.rows,
                        lifecycle: lifecycle.into(),
                        sample: o.sample * o.repeats + sample - 1,
                        chunk: o.chunk,
                        delay_us: o.delay.as_micros() as u64,
                        total_wall_ns: wall,
                        total_cpu_ns: cpu,
                        phases,
                        code_bytes: bytes,
                        receipt: r,
                        status: "completed",
                        allocation_diagnostics: allocation_delta(before, after),
                    });
                }
            }
        }
    }
    Ok(())
}
fn source_fn(case: &str) -> Result<String, String> {
    source(case, 0)
}
#[derive(Serialize)]
struct RequestRow {
    kind: &'static str,
    engine: String,
    case: String,
    sample: usize,
    request: usize,
    class: &'static str,
    rows: usize,
    concurrency: usize,
    chunk: usize,
    delay_us: u64,
    queue_ns: u64,
    service_ns: u64,
    latency_ns: u64,
    receipt: Option<i64>,
    status: String,
}
fn concurrent(o: &Options, rt: &tokio::runtime::Runtime) -> Result<(), String> {
    let source = source(&o.case, 0)?;
    let mut engines = o.engines.clone();
    if o.sample % 2 == 1 {
        engines.reverse();
    }
    for engine in engines {
        let native = if engine == "native" {
            Some(Rc::new(
                CodeUnit::compile(&source).map_err(|e| e.to_string())?,
            ))
        } else {
            None
        };
        let monty = if engine == "monty" {
            Some(compile_monty(&source)?)
        } else {
            None
        };
        // Real ready-path warmup outside the campaign. Both arms use detached I/O.
        let warm_provider = Arc::new(RequestProvider::ready(0, o.delay));
        if let Some(code) = &native {
            rt.block_on(runtime::execute(
                code.clone(),
                warm_provider,
                BATCH,
                LIMIT,
                Limits {
                    chunk: o.chunk,
                    ..Default::default()
                },
                Cancellation::default(),
                None,
            ))
            .map_err(|e| e.to_string())?;
        } else {
            rt.block_on(execute_monty(
                monty.as_ref().unwrap().clone(),
                warm_provider,
            ))?;
        }
        let sem = Arc::new(tokio::sync::Semaphore::new(o.concurrency));
        let expected_heavy = oracle(o.rows, &o.case, 0);
        let expected_short = oracle(o.rows.min(100), &o.case, 0);
        let local = tokio::task::LocalSet::new();
        reset_peak();
        let alloc_before = allocation();
        let cpu = cpu_ns();
        let start = Instant::now();
        let results = rt.block_on(local.run_until(async {
            let mut joins = Vec::with_capacity(o.requests);
            for request in 0..o.requests {
                let sem = sem.clone();
                let code = native.clone();
                let monty = monty.clone();
                let engine = engine.clone();
                let case = o.case.clone();
                let n = if o.mixed && request % 4 != 0 {
                    o.rows.min(100)
                } else {
                    o.rows
                };
                let expected = if o.mixed && request % 4 != 0 {
                    expected_short
                } else {
                    expected_heavy
                };
                let enqueued = Instant::now();
                let o = TaskOptions {
                    chunk: o.chunk,
                    delay: o.delay,
                    concurrency: o.concurrency,
                    sample: o.sample,
                    mixed: o.mixed,
                };
                joins.push(tokio::task::spawn_local(async move {
                    let permit = sem.acquire_owned().await.unwrap();
                    let admitted = Instant::now();
                    let queue_ns = admitted.duration_since(enqueued).as_nanos() as u64;
                    let provider = Arc::new(RequestProvider::ready(n, o.delay));
                    let r = if let Some(code) = code {
                        runtime::execute(
                            code,
                            provider,
                            BATCH,
                            LIMIT,
                            Limits {
                                chunk: o.chunk,
                                ..Default::default()
                            },
                            Cancellation::default(),
                            None,
                        )
                        .await
                        .map_err(|e| e.to_string())
                    } else {
                        execute_monty(monty.unwrap(), provider).await
                    };
                    let done = Instant::now();
                    drop(permit);
                    RequestRow {
                        kind: "request",
                        engine,
                        case,
                        sample: o.sample,
                        request,
                        class: if o.mixed && request % 4 != 0 {
                            "short"
                        } else {
                            "heavy"
                        },
                        rows: n,
                        concurrency: o.concurrency,
                        chunk: o.chunk,
                        delay_us: o.delay.as_micros() as u64,
                        queue_ns,
                        service_ns: done.duration_since(admitted).as_nanos() as u64,
                        latency_ns: done.duration_since(enqueued).as_nanos() as u64,
                        receipt: r.as_ref().ok().copied(),
                        status: match r {
                            Ok(v) if v == expected => "completed".into(),
                            Ok(_) => "receipt_mismatch".into(),
                            Err(e) => format!("failed: {e}"),
                        },
                    }
                }));
            }
            let mut results = Vec::with_capacity(o.requests);
            for join in joins {
                results.push(join.await.map_err(|e| e.to_string())?);
            }
            Ok::<_, String>(results)
        }))?;
        let wall = now_ns(start);
        let cpu = cpu_ns() - cpu;
        let after = allocation();
        let completed = results.iter().filter(|r| r.status == "completed").count();
        emit(
            &serde_json::json!({"kind":"concurrent", "engine":engine, "case":o.case, "sample":o.sample, "lifecycle":"prepared_shared_code_bounded_admission", "concurrency":o.concurrency, "requests":o.requests, "completed":completed, "failed":o.requests-completed, "rows":o.rows, "mixed":o.mixed, "chunk":o.chunk, "delay_us":o.delay.as_micros() as u64, "total_wall_ns":wall, "total_cpu_ns":cpu, "throughput_per_second": completed as f64 * 1e9 / wall as f64, "allocation_diagnostics":allocation_delta(alloc_before,after)}),
        );
        for r in results {
            emit(&r);
        }
        if completed != o.requests {
            return Err("campaign contains failed requests".into());
        }
    }
    Ok(())
}
fn run() -> Result<(), String> {
    let o = Options::parse()?;
    emit(
        &serde_json::json!({"kind":"identity", "schema":2, "monty":MONTY_REV, "host_os":std::env::consts::OS, "host_arch":std::env::consts::ARCH, "pid":std::process::id(), "allocation_diagnostics":cfg!(feature="allocation-diagnostics"), "cpu":"process user+system getrusage ns", "provider":"two finite sleeps; zero means ready, not timer(0)", "helper":"historical generic reference, not main comparison", "argv":std::env::args().collect::<Vec<_>>()}),
    );
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .map_err(|e| e.to_string())?;
    if o.mode == "latency" {
        latency(&o, &rt)
    } else {
        concurrent(&o, &rt)
    }
}
fn main() {
    if let Err(e) = run() {
        emit(&serde_json::json!({"kind":"failure", "status":"failed", "error":e}));
        std::process::exit(1);
    }
}

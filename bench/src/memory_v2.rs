//! Separate diagnostic executable. Never use its times as uninstrumented speed.
mod v2_common;
use minipy_native::{
    codegen::CodeUnit,
    runtime::{self, Cancellation, Limits},
};
use monty::{MontyRun, RunProgress};
use monty_types::{CompileOptions, MontyObject, PrintWriter, ResourceTracker};
use std::{
    rc::Rc,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use v2_common::*;
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
struct Options {
    engine: String,
    tasks: usize,
    rows: usize,
    bytes: Option<usize>,
    unique: bool,
    cancel: bool,
    chunk: usize,
}
impl Options {
    fn parse() -> Result<Self, String> {
        let mut o = Self {
            engine: "native".into(),
            tasks: 4,
            rows: 100,
            bytes: None,
            unique: false,
            cancel: false,
            chunk: 256,
        };
        let mut args = std::env::args().skip(1);
        while let Some(k) = args.next() {
            let v = args.next().ok_or("expected --key value")?;
            let number = || v.parse::<usize>().map_err(|e| e.to_string());
            match k.as_str() {
                "--engine" => o.engine = v,
                "--tasks" => o.tasks = number()?,
                "--rows" => o.rows = number()?,
                "--bytes" => o.bytes = Some(number()?),
                "--code" => {
                    o.unique = match v.as_str() {
                        "shared" => false,
                        "unique" => true,
                        _ => return Err("code: shared|unique".into()),
                    }
                }
                "--cancel" => {
                    o.cancel = match v.as_str() {
                        "true" => true,
                        "false" => false,
                        _ => return Err("cancel: true|false".into()),
                    }
                }
                "--chunk" => o.chunk = number()?,
                _ => return Err(format!("unknown option {k}")),
            }
        }
        if !["native", "monty"].contains(&o.engine.as_str())
            || o.tasks > 4096
            || o.rows > 1_000_000
            || o.bytes.unwrap_or(0) > 16_000_000
            || o.chunk == 0
            || o.chunk > 4096
        {
            return Err("option outside bounded diagnostic limits".into());
        }
        if o.bytes.is_some() && o.engine == "native" && o.unique {
            return Err("native raw bytes probe has no compiled code control".into());
        }
        let size = o.bytes.unwrap_or(o.rows * 16);
        if size.saturating_mul(o.tasks) > 256_000_000 {
            return Err("diagnostic payload sum >256MB (harness limit)".into());
        }
        Ok(o)
    }
}
fn snapshot(
    o: &Options,
    phase: &str,
    stats: Option<&Stats>,
    payload: usize,
    capacity: Option<usize>,
    code_bytes: Option<usize>,
) {
    // Read allocator BEFORE physical reader/JSON allocations. Requested bytes include
    // harness task handles, not JIT mmap pages. No Weak row observers: even
    // after the last strong owner drops, Weak pins the Arc allocation.
    let alloc = allocation();
    let phys = physical();
    emit(
        &serde_json::json!({"kind":"memory", "phase":phase, "engine":o.engine, "tasks":o.tasks, "rows":if o.bytes.is_none(){Some(o.rows)}else{None}, "bytes_per_task":o.bytes, "code":if o.unique{"unique"}else{"shared"}, "cancel":o.cancel, "payload_live_logical_bytes":payload, "input_capacity_bytes":capacity, "code_emitted_bytes":code_bytes, "allocation":alloc, "physical":phys, "loads":stats.map(|s|s.loads.load(Ordering::Acquire)), "records":stats.map(|s|s.records.load(Ordering::Acquire)), "finished_loads":stats.map(|s|s.finished_loads.load(Ordering::Acquire)), "finished_records":stats.map(|s|s.finished_records.load(Ordering::Acquire))}),
    );
}
async fn quotes(o: &Options) -> Result<(), String> {
    let base = source("weighted_scan", 0)?;
    let native = if o.engine == "native" && !o.unique {
        Some(Rc::new(
            CodeUnit::compile(&base).map_err(|e| e.to_string())?,
        ))
    } else {
        None
    };
    let monty = if o.engine == "monty" && !o.unique {
        Some(compile_monty(&base)?)
    } else {
        None
    };
    let code_bytes = native.as_ref().map(|c| c.code_bytes);
    let stats: Arc<Stats> = Arc::default();
    let load = Arc::new(Gate::new());
    let record = Arc::new(Gate::new());
    snapshot(o, "engine_idle", Some(&stats), 0, Some(0), code_bytes);
    reset_peak();
    let mut joins = Vec::with_capacity(o.tasks);
    let mut cancellations = Vec::with_capacity(o.tasks);
    for id in 0..o.tasks {
        let initial = if o.unique { id as i64 + 1 } else { 0 };
        let text = if o.unique {
            source("weighted_scan", initial)?
        } else {
            String::new()
        };
        let code = if o.engine == "native" {
            Some(if o.unique {
                Rc::new(CodeUnit::compile(&text).map_err(|e| e.to_string())?)
            } else {
                native.as_ref().unwrap().clone()
            })
        } else {
            None
        };
        let m = if o.engine == "monty" {
            Some(if o.unique {
                compile_monty(&text)?
            } else {
                monty.as_ref().unwrap().clone()
            })
        } else {
            None
        };
        let p = Arc::new(RequestProvider {
            n: o.rows,
            delay: Duration::ZERO,
            load_gate: Some(load.clone()),
            record_gate: Some(record.clone()),
            stats: stats.clone(),
            observe: true,
        });
        let cancel = Cancellation::default();
        cancellations.push(cancel.clone());
        let chunk = o.chunk;
        joins.push(tokio::task::spawn_local(async move {
            if let Some(code) = code {
                runtime::execute(
                    code,
                    p,
                    BATCH,
                    LIMIT,
                    Limits {
                        chunk,
                        ..Default::default()
                    },
                    cancel,
                    None,
                )
                .await
                .map_err(|e| e.to_string())
            } else {
                execute_monty(m.unwrap(), p).await
            }
        }));
    }
    barrier(&load, o.tasks).await?;
    snapshot(
        o,
        "first_wait",
        Some(&stats),
        o.rows * 16 * o.tasks,
        Some(o.rows * 16 * o.tasks),
        code_bytes,
    );
    if o.cancel {
        // Native public cancellation; Monty harness cancellation drops snapshot.
        for c in &cancellations {
            c.cancel();
        }
        if o.engine == "monty" {
            for j in &joins {
                j.abort();
            }
        }
        for j in joins {
            let r = j.await;
            if o.engine == "native" && !matches!(r,Ok(Err(ref e)) if e == "Cancelled") {
                return Err(format!("expected native cancellation, got {r:?}"));
            }
            if o.engine == "monty" && !matches!(r,Err(ref e) if e.is_cancelled()) {
                return Err(format!("expected Monty snapshot cancellation, got {r:?}"));
            }
        }
        if stats.finished_loads.load(Ordering::Acquire) != 0 {
            return Err("load completed before gate released".into());
        }
        snapshot(
            o,
            "cancelled_pending_provider",
            Some(&stats),
            o.rows * 16 * o.tasks,
            Some(o.rows * 16 * o.tasks),
            code_bytes,
        );
        load.release(o.tasks);
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        loop {
            let complete = stats.finished_loads.load(Ordering::Acquire) == o.tasks;
            if complete {
                break;
            }
            if std::time::Instant::now() > deadline {
                return Err("late completion cleanup timeout".into());
            }
            tokio::task::yield_now().await;
        }
        if stats.records.load(Ordering::Acquire) != 0 {
            return Err("cancelled load issued record".into());
        }
        snapshot(
            o,
            "late_completion_reclaimed",
            Some(&stats),
            0,
            Some(0),
            code_bytes,
        );
    } else {
        load.release(o.tasks);
        barrier(&record, o.tasks).await?;
        let payload = if o.engine == "native" {
            0
        } else {
            o.rows * 16 * o.tasks
        };
        snapshot(o, "second_wait", Some(&stats), payload, Some(0), code_bytes);

        record.release(o.tasks);
        for (id, j) in joins.into_iter().enumerate() {
            let r = j.await.map_err(|e| e.to_string())??;
            let initial = if o.unique { id as i64 + 1 } else { 0 };
            if r != oracle(o.rows, "weighted_scan", initial) {
                return Err("receipt mismatch".into());
            }
        }
        snapshot(o, "completed", Some(&stats), 0, Some(0), code_bytes);
    }
    drop(native);
    drop(monty);
    drop(cancellations);
    drop(stats);
    drop(load);
    drop(record);
    snapshot(o, "engine_released", None, 0, Some(0), None);
    Ok(())
}
fn bytes_code(initial: usize) -> Result<MontyRun, String> {
    MontyRun::new(
        format!(
            "data = await load_bytes()\ntotal = len(data) + {initial}\nawait record_total(total)\n"
        ),
        "bytes.py",
        vec!["load_bytes".into(), "record_total".into()],
        CompileOptions::default(),
    )
    .map_err(|e| e.to_string())
}
fn bytes_start(code: MontyRun) -> Result<RunProgress, String> {
    code.start(
        vec![
            MontyObject::function("load_bytes", None),
            MontyObject::function("record_total", None),
        ],
        ResourceTracker::default(),
        PrintWriter::Disabled,
    )
    .map_err(|e| e.to_string())
}
fn bytes_probe(o: &Options) -> Result<(), String> {
    let n = o.bytes.unwrap();
    let code = if o.engine == "monty" && !o.unique {
        Some(bytes_code(0)?)
    } else {
        None
    };
    snapshot(o, "engine_idle", None, 0, Some(0), None);
    reset_peak();
    // Each request has its own freshly allocated/touched payload. No clone-sharing.
    let mut payloads = Vec::with_capacity(o.tasks);
    let mut waits = Vec::with_capacity(if o.engine == "monty" { o.tasks } else { 0 });
    let mut capacity = 0;
    for id in 0..o.tasks {
        let mut v = Vec::with_capacity(n);
        v.resize(n, 0x5a);
        std::hint::black_box(&v);
        capacity += v.capacity();
        payloads.push(v);
        if o.engine == "monty" {
            let m = if o.unique {
                bytes_code(id + 1)?
            } else {
                code.as_ref().unwrap().clone()
            };
            waits.push(pending(bytes_start(m)?, "load_bytes")?);
        }
    }
    snapshot(o, "first_wait", None, n * o.tasks, Some(capacity), None);
    if o.cancel {
        drop(waits);
        snapshot(
            o,
            "cancelled_pending_provider",
            None,
            n * o.tasks,
            Some(capacity),
            None,
        );
        drop(payloads);
        snapshot(o, "late_completion_reclaimed", None, 0, Some(0), None);
    } else {
        let mut second = Vec::with_capacity(if o.engine == "monty" { o.tasks } else { 0 });
        for ((id, wait), v) in waits.into_iter().zip(payloads.drain(..)) {
            let p = resolve(id, wait, MontyObject::bytes(v))?;
            let RunProgress::FunctionCall(ref call) = p else {
                return Err("expected bytes record".into());
            };
            let total = call
                .args
                .arg(0)
                .and_then(|x| x.as_int())
                .ok_or("invalid bytes total")?;
            second.push((pending(p, "record_total")?, total));
        }
        // Native's raw provider-data probe models release at last use. It does not
        // execute bytes/str in the native language and has no native compiled code.
        payloads.clear();
        snapshot(
            o,
            "second_wait",
            None,
            if o.engine == "monty" { n * o.tasks } else { 0 },
            Some(0),
            None,
        );
        for (id, ((call, wait), total)) in second.into_iter().enumerate() {
            let initial = if o.unique { id + 1 } else { 0 };
            if total != (n + initial) as i64
                || receipt(resolve(call, wait, MontyObject::int(total))?)? != total
            {
                return Err("bytes receipt mismatch".into());
            }
        }
        snapshot(o, "completed", None, 0, Some(0), None);
    }
    drop(code);
    snapshot(o, "engine_released", None, 0, Some(0), None);
    Ok(())
}
fn run() -> Result<(), String> {
    let o = Options::parse()?;
    emit(
        &serde_json::json!({"kind":"identity","schema":2,"monty":MONTY_REV,"host_os":std::env::consts::OS,"host_arch":std::env::consts::ARCH,"pid":std::process::id(),"diagnostic":true,"payload_path":if o.bytes.is_none(){"real Quote/v1 per-request owned data"}else if o.engine=="native"{"raw Vec<u8> provider ownership probe; native str/bytes LANGUAGE UNSUPPORTED"}else{"real Monty bytes + len language execution"},"code_control":if o.bytes.is_some() && o.engine=="native"{"not applicable to raw bytes probe"}else if o.unique{"separately compile different initial literal per task"}else{"one retained prepared owner; native Rc / Monty clone shares program"},"allocator":"System requested-layout counters; excludes JIT mmap/allocator metadata","argv":std::env::args().collect::<Vec<_>>()}),
    );
    // Before engine compilation, but after CLI/diagnostic runtime setup.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .map_err(|e| e.to_string())?;
    snapshot(&o, "process_base", None, 0, Some(0), None);
    if o.bytes.is_some() {
        bytes_probe(&o)
    } else {
        rt.block_on(tokio::task::LocalSet::new().run_until(quotes(&o)))
    }
}
fn main() {
    if let Err(e) = run() {
        emit(&serde_json::json!({"kind":"failure","status":"failed","error":e}));
        std::process::exit(1);
    }
}

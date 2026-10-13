//! Shared measurement and workload support, not a runtime implementation.
#![allow(dead_code)]
use minipy_native::{
    buffer::{Quote, QuoteBuffer},
    error::TaskError,
    provider::Provider,
};
use monty::{MontyRun, RunProgress};
use monty_types::{CompileOptions, ExtFunctionResult, MontyObject, PrintWriter, ResourceTracker};
use serde::Serialize;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;

pub const BATCH: i64 = 17;
pub const LIMIT: i64 = 700;
pub const MONTY_REV: &str = "56949a4c45f51cedc7a96450782f58a8017e114d";

// Requested layout bytes, NOT allocator capacity/RSS. Track realloc old/new once.
static LIVE: AtomicU64 = AtomicU64::new(0);
static PEAK: AtomicU64 = AtomicU64::new(0);
static ALLOC: AtomicU64 = AtomicU64::new(0);
static FREED: AtomicU64 = AtomicU64::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static RECAL: AtomicU64 = AtomicU64::new(0);
pub struct CountingAllocator;
fn added(n: usize) {
    ALLOC.fetch_add(n as u64, Ordering::Relaxed);
    let live = LIVE.fetch_add(n as u64, Ordering::Relaxed) + n as u64;
    PEAK.fetch_max(live, Ordering::Relaxed);
}
fn removed(n: usize) {
    FREED.fetch_add(n as u64, Ordering::Relaxed);
    LIVE.fetch_sub(n as u64, Ordering::Relaxed);
}
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            CALLS.fetch_add(1, Ordering::Relaxed);
            added(l.size());
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            CALLS.fetch_add(1, Ordering::Relaxed);
            added(l.size());
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        removed(l.size());
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, l, n) };
        if !q.is_null() {
            RECAL.fetch_add(1, Ordering::Relaxed);
            removed(l.size());
            added(n);
        }
        q
    }
}
#[derive(Clone, Copy, Serialize, Debug)]
pub struct Allocation {
    pub live_requested_bytes: u64,
    pub peak_requested_bytes: u64,
    pub allocated_bytes: u64,
    pub freed_bytes: u64,
    pub allocation_calls: u64,
    pub realloc_calls: u64,
}
pub fn allocation() -> Allocation {
    Allocation {
        live_requested_bytes: LIVE.load(Ordering::Relaxed),
        peak_requested_bytes: PEAK.load(Ordering::Relaxed),
        allocated_bytes: ALLOC.load(Ordering::Relaxed),
        freed_bytes: FREED.load(Ordering::Relaxed),
        allocation_calls: CALLS.load(Ordering::Relaxed),
        realloc_calls: RECAL.load(Ordering::Relaxed),
    }
}
pub fn reset_peak() {
    PEAK.store(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
}
pub fn allocation_delta(before: Allocation, after: Allocation) -> Option<serde_json::Value> {
    if !cfg!(feature = "allocation-diagnostics") {
        return None;
    }
    Some(
        serde_json::json!({"allocated_bytes":after.allocated_bytes-before.allocated_bytes, "freed_bytes":after.freed_bytes-before.freed_bytes, "allocation_calls":after.allocation_calls-before.allocation_calls, "realloc_calls":after.realloc_calls-before.realloc_calls, "peak_live_increment_bytes":after.peak_requested_bytes.saturating_sub(before.live_requested_bytes)}),
    )
}
pub fn cpu_ns() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    assert_eq!(
        unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) },
        0
    );
    let u = unsafe { usage.assume_init() };
    let time = |t: libc::timeval| t.tv_sec as u64 * 1_000_000_000 + t.tv_usec as u64 * 1000;
    time(u.ru_utime) + time(u.ru_stime)
}
#[derive(Serialize)]
pub struct Physical {
    pub rss_bytes: Option<u64>,
    pub pss_bytes: Option<u64>,
    pub peak_rss_bytes: u64,
    pub method: &'static str,
}
pub fn physical() -> Physical {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    assert_eq!(
        unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) },
        0
    );
    let u = unsafe { usage.assume_init() };
    #[cfg(target_os = "linux")]
    {
        let rollup = std::fs::read_to_string("/proc/self/smaps_rollup").unwrap_or_default();
        let kb = |key: &str| {
            rollup
                .lines()
                .find_map(|line| {
                    line.strip_prefix(key)
                        .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
                })
                .map(|n| n * 1024)
        };
        Physical {
            rss_bytes: kb("Rss:"),
            pss_bytes: kb("Pss:"),
            peak_rss_bytes: u.ru_maxrss as u64 * 1024,
            method: "smaps_rollup/getrusage",
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let rss = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|n| n * 1024);
        Physical {
            rss_bytes: rss,
            pss_bytes: None,
            peak_rss_bytes: u.ru_maxrss as u64,
            method: "ps-rss/getrusage; no macOS PSS",
        }
    }
}
pub fn emit<T: Serialize>(row: &T) {
    println!("{}", serde_json::to_string(row).unwrap());
}
pub fn now_ns(start: Instant) -> u64 {
    start.elapsed().as_nanos() as u64
}

pub fn rows(n: usize) -> Vec<Quote> {
    (0..n)
        .map(|i| Quote {
            unit_price_cents: ((i * 37) % 1000) as i64,
            quantity: (i % 9) as i64 - 2,
        })
        .collect()
}
pub fn oracle(n: usize, case: &str, initial: i64) -> i64 {
    let mut total = initial;
    let mut previous = 0;
    for q in rows(n) {
        if q.quantity > 0
            && q.unit_price_cents <= LIMIT
            && (case != "stateful_branch" || q.unit_price_cents >= previous)
        {
            total = total
                .checked_add(q.unit_price_cents.checked_mul(q.quantity).unwrap())
                .unwrap();
            if case == "stateful_branch" {
                previous = q.unit_price_cents;
            }
        }
    }
    total.checked_add(BATCH).unwrap()
}
pub fn source(case: &str, initial: i64) -> Result<String, String> {
    if !["weighted_scan", "stateful_branch"].contains(&case) {
        return Err("unknown case".into());
    }
    let base = std::env::var_os("MINIPY_FIXTURES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures"));
    let source =
        std::fs::read_to_string(base.join(format!("{case}.py"))).map_err(|e| e.to_string())?;
    Ok(source.replace("total: i64 = 0", &format!("total: i64 = {initial}")))
}
// Mechanical data representation adapter: preserve user statements/awaits,
// remove native-only annotations, and use tuple fields for Monty's ordinary list.
pub fn monty_source(source: &str) -> String {
    source
        .replace(": i64", "")
        .replace(": bool", "")
        .replace(" -> i64", "")
        .replace("tools.", "")
        .replace("q.unit_price_cents", "q[0]")
        .replace("q.quantity", "q[1]")
        + "\nawait main(batch_id, price_limit)\n"
}
pub fn compile_monty(source: &str) -> Result<MontyRun, String> {
    MontyRun::new(
        monty_source(source),
        "request.py",
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
pub fn start_monty(code: MontyRun) -> Result<RunProgress, String> {
    code.start(
        vec![
            MontyObject::int(BATCH),
            MontyObject::int(LIMIT),
            MontyObject::function("load_quotes", None),
            MontyObject::function("record_total", None),
        ],
        ResourceTracker::default(),
        PrintWriter::Disabled,
    )
    .map_err(|e| e.to_string())
}
pub fn object(buffer: &QuoteBuffer) -> MontyObject {
    MontyObject::list(buffer.rows().iter().map(|q| {
        MontyObject::tuple(vec![
            MontyObject::int(q.unit_price_cents),
            MontyObject::int(q.quantity),
        ])
    }))
}
pub fn pending(progress: RunProgress, name: &str) -> Result<(u32, monty::ResolveFutures), String> {
    let RunProgress::FunctionCall(call) = progress else {
        return Err(format!("expected {name}"));
    };
    if call.function_name != name {
        return Err(format!("expected {name}, got {}", call.function_name));
    }
    let id = call.call_id;
    let RunProgress::ResolveFutures(wait) = call
        .resume_pending(PrintWriter::Disabled)
        .map_err(|e| e.to_string())?
    else {
        return Err("expected actual await suspension".into());
    };
    Ok((id, wait))
}
pub fn resolve(
    id: u32,
    wait: monty::ResolveFutures,
    value: MontyObject,
) -> Result<RunProgress, String> {
    wait.resume(
        vec![(id, ExtFunctionResult::Return(value))],
        PrintWriter::Disabled,
    )
    .map_err(|e| e.to_string())
}
pub fn record_total(progress: &RunProgress) -> Result<i64, String> {
    let RunProgress::FunctionCall(call) = progress else {
        return Err("expected record call".into());
    };
    if call.function_name != "record_total"
        || call.args.arg(0).and_then(|x| x.as_int()) != Some(BATCH)
    {
        return Err("invalid record call".into());
    }
    call.args
        .arg(1)
        .and_then(|x| x.as_int())
        .ok_or("invalid total".into())
}
pub fn receipt(progress: RunProgress) -> Result<i64, String> {
    let RunProgress::Complete(v) = progress else {
        return Err("expected completion".into());
    };
    v.as_ref().as_int().ok_or("invalid receipt".into())
}

pub struct Gate {
    sem: Semaphore,
    pub arrived: AtomicUsize,
}
impl Gate {
    pub fn new() -> Self {
        Self {
            sem: Semaphore::new(0),
            arrived: AtomicUsize::new(0),
        }
    }
    pub async fn wait(&self) {
        self.arrived.fetch_add(1, Ordering::Release);
        self.sem.acquire().await.unwrap().forget();
    }
    pub fn release(&self, n: usize) {
        self.sem.add_permits(n);
    }
}
#[derive(Default)]
pub struct Stats {
    pub loads: AtomicUsize,
    pub records: AtomicUsize,
    pub finished_loads: AtomicUsize,
    pub finished_records: AtomicUsize,
}
pub struct RequestProvider {
    pub n: usize,
    pub delay: Duration,
    pub load_gate: Option<Arc<Gate>>,
    pub record_gate: Option<Arc<Gate>>,
    pub stats: Arc<Stats>,
    pub observe: bool,
}
impl RequestProvider {
    pub fn ready(n: usize, delay: Duration) -> Self {
        Self {
            n,
            delay,
            load_gate: None,
            record_gate: None,
            stats: Arc::default(),
            observe: false,
        }
    }
    pub async fn wait_load(&self, buffer: QuoteBuffer) -> Result<QuoteBuffer, TaskError> {
        if self.observe {
            self.stats.loads.fetch_add(1, Ordering::Release);
        }

        if let Some(gate) = &self.load_gate {
            gate.wait().await;
        }
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        if self.observe {
            self.stats.finished_loads.fetch_add(1, Ordering::Release);
        }
        Ok(buffer)
    }
}
impl Provider for RequestProvider {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        let buffer = QuoteBuffer::from_quotes(rows(self.n))?;
        self.wait_load(buffer).await
    }
    async fn record_total(&self, batch: i64, total: i64) -> Result<i64, TaskError> {
        if self.observe {
            self.stats.records.fetch_add(1, Ordering::Release);
        }
        if let Some(gate) = &self.record_gate {
            gate.wait().await;
        }
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        if self.observe {
            self.stats.finished_records.fetch_add(1, Ordering::Release);
        }
        total.checked_add(batch).ok_or(TaskError::Overflow)
    }
}
// Same detached provider ownership as native; cancellation drops receiver/snapshot,
// never an already issued operation. Scan is a blocking Monty turn (no invented preemption).
pub async fn execute_monty(code: MontyRun, provider: Arc<RequestProvider>) -> Result<i64, String> {
    let progress = start_monty(code)?;
    let (id, wait) = pending(progress, "load_quotes")?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    let p = provider.clone();
    tokio::spawn(async move {
        let result = p.load_quotes(BATCH).await.map(|b| object(&b));
        let _ = tx.send(result);
    });
    let value = rx
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let progress = resolve(id, wait, value)?;
    let total = record_total(&progress)?;
    let (id, wait) = pending(progress, "record_total")?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    let p = provider.clone();
    tokio::spawn(async move {
        let _ = tx.send(p.record_total(BATCH, total).await);
    });
    let value = rx
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    receipt(resolve(id, wait, MontyObject::int(value))?)
}
pub async fn barrier(gate: &Gate, n: usize) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while gate.arrived.load(Ordering::Acquire) < n {
        if Instant::now() > deadline {
            return Err(format!(
                "barrier timeout: {}/{n}",
                gate.arrived.load(Ordering::Acquire)
            ));
        }
        tokio::task::yield_now().await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn ordinary_monty_runs_both_user_programs() {
        for case in ["weighted_scan", "stateful_branch"] {
            for n in [0, 100, 10_000] {
                let code = compile_monty(&source(case, 3).unwrap()).unwrap();
                let value =
                    execute_monty(code, Arc::new(RequestProvider::ready(n, Duration::ZERO)))
                        .await
                        .unwrap();
                assert_eq!(value, oracle(n, case, 3));
            }
        }
    }
}

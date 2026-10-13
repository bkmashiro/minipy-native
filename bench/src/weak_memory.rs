//! Explicit Weak-observer control. Not used in the unpolluted memory comparison.
use super::v2_common::{self, Gate};
use minipy_native::{
    buffer::QuoteBuffer,
    error::TaskError,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
};
use std::sync::{Arc, Mutex, atomic::Ordering};
use std::time::{Duration, Instant};
struct Observed {
    n: usize,
    gate: Arc<Gate>,
    observer: Mutex<Option<Box<dyn Fn() -> bool + Send>>>,
}
impl Provider for Observed {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        let quotes = QuoteBuffer::from_quotes(v2_common::rows(self.n))?;
        let weak = quotes.downgrade();
        *self.observer.lock().unwrap() = Some(Box::new(move || weak.upgrade().is_none()));
        Ok(quotes)
    }
    async fn record_total(&self, batch: i64, total: i64) -> Result<i64, TaskError> {
        self.gate.wait().await;
        total.checked_add(batch).ok_or(TaskError::Overflow)
    }
}
pub async fn run(source: &str, n: usize) {
    let gate = Arc::new(Gate::new());
    let p = Arc::new(Observed {
        n,
        gate: gate.clone(),
        observer: Mutex::new(None),
    });
    let code = super::native(source);
    let expected = super::expected("weighted", n);
    let start = Instant::now();
    v2_common::reset_peak();
    snapshot(n, "startup", start);
    let provider = p.clone();
    let job = tokio::task::spawn_local(async move {
        let mut limits = ScriptLimits::default();
        limits.limits.rows = n as u64;
        runtime::execute_script(
            code,
            provider,
            &[v2_common::BATCH, v2_common::LIMIT],
            limits,
            Cancellation::default(),
            None,
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(20), async {
        while gate.arrived.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        (p.observer.lock().unwrap().as_ref().unwrap())(),
        "strong owner must be gone at record"
    );
    snapshot(n, "record_wait_with_weak", start);
    gate.release(1);
    let out = job.await.unwrap();
    assert_eq!(out.status, Ok(()));
    assert_eq!(out.stdout, expected.0);
    drop(out);
    snapshot(n, "completed_with_weak", start);
    p.observer.lock().unwrap().take();
    snapshot(n, "weak_dropped", start);
}
fn snapshot(n: usize, phase: &str, start: Instant) {
    let allocation = v2_common::allocation();
    let physical = v2_common::physical();
    v2_common::emit(
        &serde_json::json!({"kind":"weak_observer_control","rows":n,"phase":phase,"allocation":allocation,"physical":physical,"elapsed_ns":start.elapsed().as_nanos() as u64}),
    );
}

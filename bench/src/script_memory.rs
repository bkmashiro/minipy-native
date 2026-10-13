//! A bounded gated lifecycle probe, separate from speed measurements.
use super::v2_common::{self, Gate, RequestProvider};
use super::{expected, monty, monty_run, native, native_run, validate};
use std::{
    rc::Rc,
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
async fn all_arrived(gate: &Gate, n: usize) {
    tokio::time::timeout(Duration::from_secs(20), async {
        while gate.arrived.load(Ordering::Acquire) < n {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("all admitted operations must reach their gate");
}
pub async fn run(engine: &str, source: &str, case: &str, n: usize, count: usize, delay: Duration) {
    if !cfg!(feature = "allocation-diagnostics") {
        panic!("hold is a separate diagnostic build");
    }
    assert!(count <= 512 && case != "scalar");
    let expected = Rc::new(expected(case, n));
    let legacy = case == "legacy";
    let load = Arc::new(Gate::new());
    let record = Arc::new(Gate::new());
    v2_common::reset_peak();
    let start = Instant::now();
    snapshot(engine, case, n, count, "startup", start);
    let mut jobs = Vec::new();
    for _ in 0..count {
        let source = source.to_owned();

        let load = load.clone();
        let record = record.clone();
        let expected = expected.clone();
        let mut p = RequestProvider::ready(n, delay);
        p.load_gate = Some(load);
        p.record_gate = Some(record);
        let p = Arc::new(p);
        // Separate concrete futures: an inactive Monty arm must not enlarge a
        // native task's waiting allocation (Rust async union layout).
        if engine == "native" {
            jobs.push(tokio::task::spawn_local(async move {
                let result = native_run(native(&source), p, legacy).await.unwrap();
                validate(&result, &expected, legacy);
            }));
        } else {
            jobs.push(tokio::task::spawn_local(async move {
                let result = monty_run(monty(&source, legacy), p).await.unwrap();
                validate(&result, &expected, legacy);
            }));
        }
    }
    all_arrived(&load, count).await;
    snapshot(engine, case, n, count, "load_wait", start);
    load.release(count);
    all_arrived(&record, count).await;
    snapshot(engine, case, n, count, "record_wait", start);
    record.release(count);
    for j in jobs {
        j.await.unwrap();
    }
    snapshot(engine, case, n, count, "reclaimed", start);
}
fn snapshot(engine: &str, case: &str, n: usize, count: usize, phase: &str, start: Instant) {
    let allocation = v2_common::allocation();
    let physical = v2_common::physical();
    v2_common::emit(
        &serde_json::json!({"kind":"script_memory","engine":engine,"case":case,"rows":n,"count":count,"phase":phase,"allocation":allocation,"physical":physical,"elapsed_ns":start.elapsed().as_nanos() as u64}),
    );
}

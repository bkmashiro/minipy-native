//! Fixed assembly versus Rust reference, never source compilation.
#[path = "../assembly_reference.rs"]
mod assembly;
#[path = "../v2_common.rs"]
mod common;
use minipy_native::{
    buffer::QuoteBuffer,
    fixture_reference::{self, Fixture},
    runtime::{Cancellation, ScriptLimits},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
#[cfg(feature = "allocation-diagnostics")]
#[global_allocator]
static ALLOC: common::CountingAllocator = common::CountingAllocator;

async fn execute(arm: &str, stateful: bool, p: Arc<common::RequestProvider>) -> String {
    let out = if arm == "assembly" {
        assembly::execute(
            stateful,
            p,
            &[common::BATCH, common::LIMIT],
            ScriptLimits::default(),
            assembly::Cancellation::default(),
        )
        .await
    } else {
        fixture_reference::execute(
            if stateful {
                Fixture::Stateful
            } else {
                Fixture::Weighted
            },
            p,
            &[common::BATCH, common::LIMIT],
            ScriptLimits::default(),
            Cancellation::default(),
            None,
        )
        .await
    };
    out.status.unwrap();
    out.stdout
}
fn main() {
    // Bound to these two fixed success programs, not arbitrary source.
    let _bodies = [include_str!("../../../fixtures/common_weighted.py"),
        include_str!("../../../fixtures/common_stateful.py")];
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(
        args.len(),
        7,
        "assembly|rust kernel|full|hold weighted|stateful ROWS REPS DELAY_US"
    );
    let (arm, mode, case) = (&args[1], &args[2], &args[3]);
    assert!(["assembly", "rust"].contains(&arm.as_str()));
    assert!(["weighted", "stateful"].contains(&case.as_str()));
    let n: usize = args[4].parse().unwrap();
    let reps: usize = args[5].parse().unwrap();
    let delay: u64 = args[6].parse().unwrap();
    assert!(n <= 100000 && reps > 0);
    let stateful = case == "stateful";
    let expected = common::oracle(
        n,
        if stateful {
            "stateful_branch"
        } else {
            "weighted_scan"
        },
        0,
    );
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    if mode == "hold" {
        assert!(cfg!(feature = "allocation-diagnostics") && reps <= 32);
        tokio::task::LocalSet::new().block_on(&rt, async {
            let load=Arc::new(common::Gate::new()); let record=Arc::new(common::Gate::new());
            let snap=|phase:&str|common::emit(&serde_json::json!({"kind":"assembly-hold","engine":arm,"case":case,"rows":n,"count":reps,"code":"static","phase":phase,"allocation":common::allocation(),"physical":common::physical(),"kernel_code_bytes":assembly::code_bytes(),"numeric_state_bytes":16,"input_payload_bytes":if phase=="load_wait" {n*16*reps} else {0}}));
            snap("startup");
            let mut jobs=Vec::new();
            for _ in 0..reps {
                let mut p=common::RequestProvider::ready(n,Duration::ZERO);
                p.load_gate=Some(load.clone()); p.record_gate=Some(record.clone());
                let arm=arm.clone();
                jobs.push(tokio::task::spawn_local(async move {assert_eq!(execute(&arm,stateful,Arc::new(p)).await,format!("{expected}\n"))}));
            }
            common::barrier(&load,reps).await.unwrap(); snap("load_wait"); load.release(reps);
            common::barrier(&record,reps).await.unwrap(); snap("record_wait"); record.release(reps);
            for j in jobs {j.await.unwrap();} snap("reclaimed");
        });
        return;
    }
    assert!(!cfg!(feature = "allocation-diagnostics"));
    // Only kernel-only timing reuses a prevalidated input. Full-path requests
    // generate their own input inside Provider; retain no unused extra fixture.
    let rows = (mode == "kernel").then(|| QuoteBuffer::from_quotes(common::rows(n)).unwrap());
    for sample in 0..reps {
        let cpu = common::cpu_ns();
        let start = Instant::now();
        if mode == "kernel" {
            let (mut sum, mut previous) = (0, 0);
            for chunk in std::hint::black_box(rows.as_ref().unwrap().rows()).chunks(256) {
                if arm == "assembly" {
                    assembly::kernel(chunk, &mut sum, &mut previous, common::LIMIT, stateful)
                        .unwrap();
                } else {
                    assembly::rust_kernel(chunk, &mut sum, &mut previous, common::LIMIT, stateful)
                        .unwrap();
                }
            }
            std::hint::black_box((sum, previous));
            let wall_ns = common::now_ns(start);
            let cpu_ns = common::cpu_ns() - cpu;
            assert_eq!(sum + common::BATCH, expected);
            common::emit(
                &serde_json::json!({"kind":"assembly-speed","engine":arm,"mode":mode,"case":case,"rows":n,"sample":sample,"wall_ns":wall_ns,"cpu_ns":cpu_ns,"kernel_code_bytes":assembly::code_bytes(),"completed":true}),
            );
        } else {
            assert_eq!(mode, "full");
            let p = Arc::new(common::RequestProvider::ready(
                n,
                Duration::from_micros(delay),
            ));
            let result = rt.block_on(execute(arm, stateful, p));
            let wall_ns = common::now_ns(start);
            let cpu_ns = common::cpu_ns() - cpu;
            assert_eq!(result, format!("{expected}\n"));
            common::emit(
                &serde_json::json!({"kind":"assembly-speed","engine":arm,"mode":mode,"case":case,"rows":n,"delay_us":delay,"sample":sample,"wall_ns":wall_ns,"cpu_ns":cpu_ns,"kernel_code_bytes":assembly::code_bytes(),"completed":true}),
            );
        }
    }
}

#[cfg(all(test, any(target_arch = "aarch64", target_arch = "x86_64"), target_os = "linux"))]
mod tests {
    use super::*;
    use minipy_native::error::TaskError;
    use std::sync::atomic::Ordering;
    #[tokio::test]
    async fn errors_and_owner_release_before_record() {
        use minipy_native::provider::{TestProvider, Delays};
        for which in 0..6 {
            let mut options = ScriptLimits::default();
            match which {
                0 => options.limits.allow_load = false,
                1 => options.tool_calls = 1,
                2 => options.limits.buffer_bytes = 0,
                3 => options.allow_print = false,
                4 => options.stdout_bytes = 0,
                _ => options.limits.chunk = 0,
            }
            let p = Arc::new(TestProvider::new(common::rows(1), Delays::default()));
            let out = assembly::execute(false, p.clone(), &[17,700], options, assembly::Cancellation::default()).await;
            assert_eq!(out.status, Err(if which == 0 || which == 3 { TaskError::Permission } else { TaskError::Budget }));
            assert!(out.stdout.is_empty());
            if which < 3 || which == 5 { assert_eq!(p.record_count(), 0); }
        }
        for (load, record) in [(true,false),(false,true)] {
            let p = Arc::new(TestProvider::new(common::rows(1), Delays::default()).with_failures(load,record));
            let out = assembly::execute(false,p,&[17,700],ScriptLimits::default(),assembly::Cancellation::default()).await;
            assert!(matches!(out.status,Err(TaskError::Provider(_))));
            assert!(out.stdout.is_empty());
        }
        let p = Arc::new(TestProvider::new(common::rows(1), Delays { load:Duration::ZERO, record:Duration::from_millis(30) }));
        let pp = p.clone();
        let job = tokio::spawn(async move { assembly::execute(false,pp,&[17,700],ScriptLimits::default(),assembly::Cancellation::default()).await });
        tokio::time::timeout(Duration::from_secs(1), async { while p.record_count()==0 { tokio::task::yield_now().await; } }).await.unwrap();
        assert!(p.last_rows().unwrap().upgrade().is_none());
        job.await.unwrap().status.unwrap();
    }
    #[tokio::test]
    async fn full_contract_budget_permission_cancel_and_late_completion() {
        for stateful in [false, true] {
            for n in [0, 1, 100, 10000] {
                for chunk in [1, 2, 256, 4096] {
                    let mut options = ScriptLimits::default();
                    options.limits.chunk = chunk;
                    let a = assembly::execute(
                        stateful,
                        Arc::new(common::RequestProvider::ready(n, Duration::ZERO)),
                        &[17, 700],
                        options,
                        assembly::Cancellation::default(),
                    )
                    .await;
                    let b = fixture_reference::execute(
                        if stateful {
                            Fixture::Stateful
                        } else {
                            Fixture::Weighted
                        },
                        Arc::new(common::RequestProvider::ready(n, Duration::ZERO)),
                        &[17, 700],
                        options,
                        Cancellation::default(),
                        None,
                    )
                    .await;
                    assert_eq!((a.status, a.stdout), (b.status, b.stdout));
                }
            }
        }
        for error in [TaskError::Budget, TaskError::Permission] {
            let mut options = ScriptLimits::default();
            if error == TaskError::Budget {
                options.limits.rows = 99;
            } else {
                options.limits.allow_record = false;
            }
            let mut p = common::RequestProvider::ready(100, Duration::ZERO);
            p.observe = true;
            let stats = p.stats.clone();
            let result = assembly::execute(
                false,
                Arc::new(p),
                &[17, 700],
                options,
                assembly::Cancellation::default(),
            )
            .await;
            assert_eq!(result.status, Err(error));
            assert_eq!(stats.records.load(Ordering::Acquire), 0);
        }
        let load = Arc::new(common::Gate::new());
        let mut p = common::RequestProvider::ready(100, Duration::ZERO);
        p.observe = true;
        p.load_gate = Some(load.clone());
        let stats = p.stats.clone();
        let cancel = assembly::Cancellation::default();
        let c = cancel.clone();
        let job = tokio::spawn(async move {
            assembly::execute(false, Arc::new(p), &[17, 700], ScriptLimits::default(), c).await
        });
        common::barrier(&load, 1).await.unwrap();
        cancel.cancel();
        let result = tokio::time::timeout(Duration::from_secs(1), job)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.status, Err(TaskError::Cancelled));
        assert_eq!(stats.records.load(Ordering::Acquire), 0);
        load.release(1);
        while stats.finished_loads.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
        assert_eq!(stats.records.load(Ordering::Acquire), 0);
    }
}

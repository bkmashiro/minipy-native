use minipy_native::{
    buffer::{Quote, QuoteBuffer, QuoteWeak},
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    ir::Type,
    provider::{Delays, Provider, TestProvider},
    runtime::{self, Cancellation, ScriptLimits, ScriptOutcome, Trace},
};
use std::{
    cell::RefCell,
    future::{Future, poll_fn},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::Poll,
    time::Duration,
};
use tokio::sync::Semaphore;
const INPUTS: &[(&str, Type)] = &[("batch_id", Type::I64), ("price_limit", Type::I64)];
fn rows() -> Vec<Quote> {
    vec![
        Quote {
            unit_price_cents: 12,
            quantity: 3,
        },
        Quote {
            unit_price_cents: 800,
            quantity: 1,
        },
        Quote {
            unit_price_cents: -1,
            quantity: 2,
        },
    ]
}
async fn differential(
    source: &str,
    schema: &[(&str, Type)],
    values: &[i64],
    rows: Vec<Quote>,
    options: ScriptLimits,
) -> ScriptOutcome {
    let native = Rc::new(CodeUnit::compile_script(source, schema).unwrap());
    let interpreted = Rc::new(InterpreterUnit::prepare_script(source, schema).unwrap());
    let a = Arc::new(TestProvider::new(rows.clone(), Delays::default()));
    let b = Arc::new(TestProvider::new(rows, Delays::default()));
    let ta: Trace = Rc::new(RefCell::new(Vec::new()));
    let tb: Trace = Rc::new(RefCell::new(Vec::new()));
    let x = runtime::execute_script(
        native,
        a.clone(),
        values,
        options,
        Cancellation::default(),
        Some(ta.clone()),
    )
    .await;
    let y = interpreter::execute_script(
        interpreted,
        b.clone(),
        values,
        options,
        Cancellation::default(),
        Some(tb.clone()),
    )
    .await;
    assert_eq!(x.status, y.status, "{source}");
    assert_eq!(x.stdout, y.stdout, "{source}");
    assert_eq!(
        (a.load_count(), a.record_count()),
        (b.load_count(), b.record_count())
    );
    let trace = |t: &Trace| {
        t.borrow()
            .iter()
            .map(|e| (e.phase, e.buffer_bytes, e.remaining, e.index))
            .collect::<Vec<_>>()
    };
    assert_eq!(trace(&ta), trace(&tb), "trace {source}");
    y
}
#[tokio::test]
async fn empty_else_joins_nested_scan_and_tool_continuations() {
    let source = "total=0\nif enabled:\n    print(1)\n    if enabled:\n        print(2)\nquotes=tools.load_quotes(7)\nfor q in quotes:\n    if q.quantity > 0:\n        if q.unit_price_cents < 700:\n            total += q.quantity\n    if q.quantity < 0:\n        print(9)\nreceipt=tools.record_total(7,total)\nprint(receipt)";
    for enabled in [0, 1] {
        for chunk in [1, 2, 256] {
            let mut limits = ScriptLimits::default();
            limits.limits.chunk = chunk;
            let out = differential(
                source,
                &[("enabled", Type::Bool)],
                &[enabled],
                rows(),
                limits,
            )
            .await;
            assert_eq!(out.status, Ok(()));
            assert_eq!(out.stdout, if enabled == 0 { "12\n" } else { "1\n2\n12\n" });
        }
    }
}
#[tokio::test]
async fn scripts_and_sequential_conditional_tools() {
    for file in [
        include_str!("../fixtures/scalar_script.py"),
        include_str!("../fixtures/scan_script.py"),
        include_str!("../fixtures/weighted_script.py"),
    ] {
        differential(file, INPUTS, &[7, 700], rows(), ScriptLimits::default()).await;
    }
    for flag in [0, 1] {
        let s = "if enabled:\n    x = tools.record_total(3,5)\nelse:\n    x = tools.record_total(3,1)\nprint(x)\nfirst = tools.load_quotes(x)\nacc = 0\nfor item in first:\n    amount = item.quantity * item.unit_price_cents\n    acc += amount\nreceipt = tools.record_total(x,acc)\nsecond = tools.load_quotes(receipt)\nfor row in second:\n    amount = row.quantity\n    acc += amount\nprint(acc)\nprint(tools.record_total(x,acc))";
        for chunk in [1, 2, 256] {
            let mut l = ScriptLimits::default();
            l.limits.chunk = chunk;
            differential(s, &[("enabled", Type::Bool)], &[flag], rows(), l).await;
        }
    }
}
#[tokio::test]
async fn scalar_short_circuit_error_order_and_stdout() {
    for s in [
        "",
        "True",
        "1 + 2",
        "x=3\nx > 0",
        "print(-9223372036854775808)",
        "print(False and 9223372036854775807 + 1 > 0)\nprint(True or 9223372036854775807 + 1 > 0)",
        "print(9)\nprint(9223372036854775807 + 1)\ntools.record_total(1,2)",
        "print(9)\n1 - -9223372036854775808",
        "print(9)\n9223372036854775807 * 2",
        "print(not False)\nprint(2 != 3)\nprint(2 >= 3)",
    ] {
        differential(s, &[], &[], rows(), ScriptLimits::default()).await;
    }
    let mut l = ScriptLimits {
        stdout_bytes: 2,
        ..ScriptLimits::default()
    };
    let out = differential("print(1)\nprint(2)", &[], &[], rows(), l).await;
    assert_eq!(out.stdout, "1\n");
    assert_eq!(out.status, Err(TaskError::Budget));
    l.allow_print = false;
    differential("print(9223372036854775807 + 1)", &[], &[], rows(), l).await;
    differential("print(flag)", &[("flag", Type::Bool)], &[2], rows(), l).await;
    differential("print(flag)", &[("flag", Type::Bool)], &[], rows(), l).await;
}
#[tokio::test]
async fn budget_across_yields_tools_live_owner_and_empty_scan() {
    let s = "first=tools.load_quotes(1)\nacc=0\nfor row in first:\n    acc += row.quantity\nprint(acc)\ntools.record_total(1,acc)\nsecond=tools.load_quotes(2)\nfor row in second:\n    amount=row.quantity\n    acc += amount\nfor row in first:\n    acc += row.quantity\nprint(acc)";
    for n in [0, 1, 3] {
        for fuel in [0, 1, 5, 9] {
            for chunk in [1, 2, 256] {
                let mut l = ScriptLimits::default();
                l.limits.rows = fuel;
                l.limits.chunk = chunk;
                differential(s, &[], &[], rows()[..n].to_vec(), l).await;
            }
        }
    }
    for calls in [0, 1, 2, 3] {
        let l = ScriptLimits {
            tool_calls: calls,
            ..ScriptLimits::default()
        };
        differential(s, &[], &[], rows(), l).await;
    }
    for bytes in [0, 47, 48, 95, 96] {
        let mut l = ScriptLimits::default();
        l.limits.buffer_bytes = bytes;
        differential(s, &[], &[], rows(), l).await;
    }
    for (load, record) in [(false, true), (true, false)] {
        let mut l = ScriptLimits::default();
        l.limits.allow_load = load;
        l.limits.allow_record = record;
        differential(s, &[], &[], rows(), l).await;
    }
    for chunk in [0, 4097] {
        let mut l = ScriptLimits::default();
        l.limits.chunk = chunk;
        differential(s, &[], &[], rows(), l).await;
    }
}
#[tokio::test]
async fn scan_next_chunk_boundary_and_final_partial_chunk() {
    let source = "q=tools.load_quotes(1)\ntotal=0\nfor row in q:\n    total += row.quantity\nprint(total)\ntools.record_total(1,total)";
    for chunk in [64usize, 256, 4096] {
        for n in [0, 1, chunk - 1, chunk, chunk + 1, 2 * chunk, 2 * chunk + 1] {
            for fuel in [n.saturating_sub(1), n] {
                let mut limits = ScriptLimits::default();
                limits.limits.chunk = chunk;
                limits.limits.rows = fuel as u64;
                let out = differential(
                    source,
                    &[],
                    &[],
                    vec![
                        Quote {
                            unit_price_cents: 1,
                            quantity: 1
                        };
                        n
                    ],
                    limits,
                )
                .await;
                assert_eq!(
                    out.status,
                    if fuel < n {
                        Err(TaskError::Budget)
                    } else {
                        Ok(())
                    }
                );
                assert_eq!(
                    out.stdout,
                    if fuel < n {
                        String::new()
                    } else {
                        format!("{n}\n")
                    }
                );
            }
        }
    }
}
#[tokio::test]
async fn overflow_filter_and_loop_stdout_bound() {
    let s = "q=tools.load_quotes(1)\ntotal=0\nfor row in q:\n    if row.quantity > 0 and row.unit_price_cents < 700:\n        total += row.quantity * row.unit_price_cents\nprint(total)\ntools.record_total(1,total)";
    differential(
        s,
        &[],
        &[],
        vec![Quote {
            unit_price_cents: i64::MAX,
            quantity: 2,
        }],
        ScriptLimits::default(),
    )
    .await;
    assert_eq!(
        differential(
            s,
            &[],
            &[],
            vec![Quote {
                unit_price_cents: 600,
                quantity: i64::MAX
            }],
            ScriptLimits::default()
        )
        .await
        .status,
        Err(TaskError::Overflow)
    );
    let mut l = ScriptLimits::default();
    l.limits.chunk = 1;
    l.stdout_bytes = 4;
    assert_eq!(
        differential(
            "q=tools.load_quotes(1)\nfor row in q:\n    print(row.quantity)",
            &[],
            &[],
            rows(),
            l
        )
        .await
        .stdout,
        "3\n1\n"
    );
}
struct Gate {
    rows: Vec<Quote>,
    load_gate: Semaphore,
    record_gate: Semaphore,
    loads: AtomicUsize,
    records: AtomicUsize,
    completed: AtomicUsize,
    weak: Mutex<Option<QuoteWeak>>,
}
impl Gate {
    fn new() -> Self {
        Self::with_rows(rows())
    }
    fn with_rows(rows: Vec<Quote>) -> Self {
        Self {
            rows,
            load_gate: Semaphore::new(0),
            record_gate: Semaphore::new(0),
            loads: AtomicUsize::new(0),
            records: AtomicUsize::new(0),
            completed: AtomicUsize::new(0),
            weak: Mutex::new(None),
        }
    }
}
impl Provider for Gate {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        self.load_gate.acquire().await.unwrap().forget();
        let b = QuoteBuffer::from_quotes(self.rows.clone())?;
        *self.weak.lock().unwrap() = Some(b.downgrade());
        self.completed.fetch_add(1, Ordering::SeqCst);
        Ok(b)
    }
    async fn record_total(&self, b: i64, t: i64) -> Result<i64, TaskError> {
        self.records.fetch_add(1, Ordering::SeqCst);
        self.record_gate.acquire().await.unwrap().forget();
        self.completed.fetch_add(1, Ordering::SeqCst);
        Ok(b + t)
    }
}
async fn until(counter: &AtomicUsize, n: usize) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while counter.load(Ordering::SeqCst) != n {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("gate did not reach {n}"));
}
#[tokio::test]
async fn real_await_cancellation_late_completion_release_and_stdout() {
    for native in [false, true] {
        for during_load in [false, true] {
            tokio::task::LocalSet::new().run_until(async {
            let s="print(7)\nq=tools.load_quotes(1)\ntotal=0\nfor row in q:\n    total += row.quantity\nprint(total)\ntools.record_total(1,total)\nprint(9)";
            let p=Arc::new(Gate::new()); let cancel=Cancellation::default(); let c=cancel.clone(); let pp=p.clone();
            let job=tokio::task::spawn_local(async move { if native { runtime::execute_script(Rc::new(CodeUnit::compile_script(s,&[]).unwrap()),pp,&[],ScriptLimits::default(),c,None).await } else { interpreter::execute_script(Rc::new(InterpreterUnit::prepare_script(s,&[]).unwrap()),pp,&[],ScriptLimits::default(),c,None).await } });
            until(&p.loads,1).await;
            if !during_load { p.load_gate.add_permits(1); until(&p.records,1).await; assert!(p.weak.lock().unwrap().as_ref().unwrap().upgrade().is_none()); }
            cancel.cancel(); let out=job.await.unwrap(); assert_eq!(out.status,Err(TaskError::Cancelled)); assert_eq!(out.stdout,if during_load {"7\n"} else {"7\n6\n"});
            if during_load { p.load_gate.add_permits(1); until(&p.completed,1).await; assert_eq!(p.records.load(Ordering::SeqCst),0); assert!(p.weak.lock().unwrap().as_ref().unwrap().upgrade().is_none()); } else { p.record_gate.add_permits(1); until(&p.completed,2).await; }
        }).await;
        }
    }
}
#[tokio::test]
async fn chunk_cancellation_and_shared_unit_state() {
    let s = "q=tools.load_quotes(1)\ntotal=0\nfor row in q:\n    total+=row.quantity\nprint(total)\ntools.record_total(1,total)";
    let unit = Rc::new(InterpreterUnit::prepare_script(s, &[]).unwrap());
    let cancel = Cancellation::default();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let p = Arc::new(Gate::with_rows(vec![
        Quote {
            unit_price_cents: 1,
            quantity: 1
        };
        1000
    ]));
    let mut l = ScriptLimits::default();
    l.limits.chunk = 1;
    let job = interpreter::execute_script(
        unit.clone(),
        p.clone(),
        &[],
        l,
        cancel.clone(),
        Some(trace.clone()),
    );
    tokio::pin!(job);
    tokio::select! {
        out = &mut job => panic!("task finished before load gate: {:?}", out.status),
        _ = until(&p.loads, 1) => {}
    }
    assert!(trace.borrow().iter().all(|e| e.phase != "yield"));
    p.load_gate.add_permits(1);
    // Drive the actual task future only to its first chunk suspension, then
    // stop polling it. This test-only handshake cannot race a resumed scan.
    tokio::time::timeout(
        Duration::from_secs(5),
        poll_fn(|cx| {
            assert!(
                job.as_mut().poll(cx).is_pending(),
                "task finished before chunk yield"
            );
            if trace.borrow().iter().any(|e| e.phase == "yield") {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }),
    )
    .await
    .expect("task did not reach chunk yield");
    {
        let events = trace.borrow();
        let yields: Vec<_> = events.iter().filter(|e| e.phase == "yield").collect();
        assert_eq!(yields.len(), 1);
        assert_eq!(
            (yields[0].index, yields[0].remaining, yields[0].buffer_bytes),
            (1, l.limits.rows - 1, 16_000)
        );
    }
    assert!(p.weak.lock().unwrap().as_ref().unwrap().upgrade().is_some());
    cancel.cancel();
    let out = job.await;
    assert_eq!(out.status, Err(TaskError::Cancelled));
    assert!(out.stdout.is_empty());
    assert_eq!(p.records.load(Ordering::SeqCst), 0);
    assert!(p.weak.lock().unwrap().as_ref().unwrap().upgrade().is_none());
    assert_eq!(
        trace.borrow().iter().filter(|e| e.phase == "yield").count(),
        1
    );
    for n in [1, 3] {
        let p = Arc::new(TestProvider::new(rows()[..n].to_vec(), Delays::default()));
        let out = interpreter::execute_script(
            unit.clone(),
            p,
            &[],
            ScriptLimits::default(),
            Cancellation::default(),
            None,
        )
        .await;
        assert_eq!(out.status, Ok(()));
        assert_eq!(out.stdout, if n == 1 { "3\n" } else { "6\n" });
    }
}
#[test]
fn sealed_preparation_rejects_unsupported_and_unbound_source() {
    for s in [
        "print('x')",
        "x = missing",
        "while True:\n    pass",
        "q=tools.load_quotes(1)\nfor row in q:\n    tools.record_total(1,2)",
    ] {
        assert!(InterpreterUnit::prepare_script(s, &[]).is_err());
    }
}

#[tokio::test]
async fn provider_failures_and_cancelled_entry_do_not_run_later_effects() {
    let source = "print(7)\nq=tools.load_quotes(1)\ntotal=0\nfor row in q:\n    total += row.quantity\nprint(total)\ntools.record_total(1,total)\nprint(9)";
    for (load, record) in [(true, false), (false, true)] {
        let native = Rc::new(CodeUnit::compile_script(source, &[]).unwrap());
        let unit = Rc::new(InterpreterUnit::prepare_script(source, &[]).unwrap());
        let a = Arc::new(TestProvider::new(rows(), Delays::default()).with_failures(load, record));
        let b = Arc::new(TestProvider::new(rows(), Delays::default()).with_failures(load, record));
        let x = runtime::execute_script(
            native,
            a.clone(),
            &[],
            ScriptLimits::default(),
            Cancellation::default(),
            None,
        )
        .await;
        let y = interpreter::execute_script(
            unit,
            b.clone(),
            &[],
            ScriptLimits::default(),
            Cancellation::default(),
            None,
        )
        .await;
        assert_eq!(x.status, y.status);
        assert!(y.status.is_err());
        assert_eq!(x.stdout, y.stdout);
        assert_eq!(
            (a.load_count(), a.record_count()),
            (b.load_count(), b.record_count())
        );
    }
    let p = Arc::new(TestProvider::new(rows(), Delays::default()));
    let cancel = Cancellation::default();
    cancel.cancel();
    let out = interpreter::execute_script(
        Rc::new(InterpreterUnit::prepare_script(source, &[]).unwrap()),
        p.clone(),
        &[],
        ScriptLimits::default(),
        cancel,
        None,
    )
    .await;
    assert_eq!(out.status, Err(TaskError::Cancelled));
    assert!(out.stdout.is_empty());
    assert_eq!(p.load_count(), 0);
}

#[tokio::test]
async fn dropped_task_receiver_releases_late_load_without_resuming() {
    tokio::task::LocalSet::new().run_until(async {
        let p = Arc::new(Gate::new()); let pp = p.clone();
        let unit = Rc::new(InterpreterUnit::prepare_script("q=tools.load_quotes(1)\nfor row in q:\n    print(row.quantity)\ntools.record_total(1,2)", &[]).unwrap());
        let weak_unit = Rc::downgrade(&unit);
        let job = tokio::task::spawn_local(async move {interpreter::execute_script(unit, pp, &[], ScriptLimits::default(), Cancellation::default(), None).await});
        until(&p.loads, 1).await; job.abort(); assert!(matches!(job.await, Err(error) if error.is_cancelled()));
        assert!(weak_unit.upgrade().is_none()); p.load_gate.add_permits(1); until(&p.completed, 1).await;
        assert!(p.weak.lock().unwrap().as_ref().unwrap().upgrade().is_none()); assert_eq!(p.records.load(Ordering::SeqCst), 0);
    }).await;
}

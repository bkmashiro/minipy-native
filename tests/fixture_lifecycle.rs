#![cfg(feature = "fixture-reference")]
use minipy_native::{
    buffer::{Quote, QuoteBuffer, QuoteWeak},
    codegen::CodeUnit,
    error::TaskError,
    fixture_reference::{self, Fixture},
    interpreter::{self, InterpreterUnit},
    ir::Type,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits, ScriptOutcome, Trace},
};
use std::{
    cell::RefCell,
    future::{Future, poll_fn},
    pin::Pin,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::Poll,
};
use tokio::sync::Semaphore;
struct Gated {
    load: Semaphore,
    record: Semaphore,
    completed: Semaphore,
    rows: Mutex<Option<QuoteWeak>>,
    records: AtomicUsize,
    fail_load: bool,
    fail_record: bool,
}
impl Provider for Gated {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        self.load.acquire().await.unwrap().forget();
        if self.fail_load {
            self.completed.add_permits(1);
            return Err(TaskError::Provider("load".into()));
        }
        let b = QuoteBuffer::from_quotes(vec![Quote {
            unit_price_cents: 3,
            quantity: 2,
        }])?;
        *self.rows.lock().unwrap() = Some(b.downgrade());
        self.completed.add_permits(1);
        Ok(b)
    }
    async fn record_total(&self, batch: i64, total: i64) -> Result<i64, TaskError> {
        self.records.fetch_add(1, Ordering::Relaxed);
        self.record.acquire().await.unwrap().forget();
        self.completed.add_permits(1);
        if self.fail_record {
            Err(TaskError::Provider("record".into()))
        } else {
            total.checked_add(batch).ok_or(TaskError::Overflow)
        }
    }
}
fn provider(fail_load: bool, fail_record: bool) -> Arc<Gated> {
    Arc::new(Gated {
        load: Semaphore::new(0),
        record: Semaphore::new(0),
        completed: Semaphore::new(0),
        rows: Mutex::new(None),
        records: AtomicUsize::new(0),
        fail_load,
        fail_record,
    })
}
fn execution(
    engine: usize,
    p: Arc<Gated>,
    cancel: Cancellation,
    trace: Trace,
) -> Pin<Box<dyn Future<Output = ScriptOutcome>>> {
    let s = include_str!("../fixtures/weighted_script.py");
    let schema = &[("batch_id", Type::I64), ("price_limit", Type::I64)];
    match engine {
        0 => Box::pin(runtime::execute_script(
            Rc::new(CodeUnit::compile_script(s, schema).unwrap()),
            p,
            &[17, 700],
            ScriptLimits::default(),
            cancel,
            Some(trace),
        )),
        1 => Box::pin(interpreter::execute_script(
            Rc::new(InterpreterUnit::prepare_script(s, schema).unwrap()),
            p,
            &[17, 700],
            ScriptLimits::default(),
            cancel,
            Some(trace),
        )),
        _ => Box::pin(fixture_reference::execute(
            Fixture::Weighted,
            p,
            &[17, 700],
            ScriptLimits::default(),
            cancel,
            Some(trace),
        )),
    }
}
async fn phase(f: &mut Pin<Box<dyn Future<Output = ScriptOutcome>>>, trace: &Trace, name: &str) {
    poll_fn(|cx| match f.as_mut().poll(cx) {
        Poll::Ready(_) => panic!("completed before gate"),
        Poll::Pending if trace.borrow().iter().any(|e| e.phase == name) => Poll::Ready(()),
        Poll::Pending => Poll::Pending,
    })
    .await;
}
#[tokio::test]
async fn detached_receiver_drop_cancel_last_use_and_effect_errors() {
    for engine in 0..3 {
        for at_record in [false, true] {
            for cancel_receiver in [false, true] {
                let p = provider(false, false);
                let cancel = Cancellation::default();
                let trace: Trace = Rc::new(RefCell::new(Vec::new()));
                let mut f = execution(engine, p.clone(), cancel.clone(), trace.clone());
                phase(&mut f, &trace, "load_wait").await;
                if at_record {
                    p.load.add_permits(1);
                    phase(&mut f, &trace, "record_wait").await;
                    p.completed.acquire().await.unwrap().forget();
                    assert!(p.rows.lock().unwrap().as_ref().unwrap().upgrade().is_none());
                }
                if cancel_receiver {
                    cancel.cancel();
                    let out = f.await;
                    assert_eq!(out.status, Err(TaskError::Cancelled));
                    assert!(out.stdout.is_empty());
                } else {
                    drop(f);
                }
                if at_record {
                    p.record.add_permits(1);
                } else {
                    p.load.add_permits(1);
                }
                p.completed.acquire().await.unwrap().forget();
                tokio::task::yield_now().await;
                assert_eq!(p.records.load(Ordering::Relaxed), usize::from(at_record));
                assert!(p.rows.lock().unwrap().as_ref().unwrap().upgrade().is_none());
            }
        }
        for (load, record) in [(true, false), (false, true)] {
            let p = provider(load, record);
            p.load.add_permits(1);
            p.record.add_permits(1);
            let out = execution(
                engine,
                p,
                Cancellation::default(),
                Rc::new(RefCell::new(Vec::new())),
            )
            .await;
            assert_eq!(
                out.status,
                Err(TaskError::Provider(
                    if load { "load" } else { "record" }.into()
                ))
            );
            assert!(out.stdout.is_empty());
        }
    }
}

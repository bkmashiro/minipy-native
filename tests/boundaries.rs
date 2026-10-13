use minipy_native::{
    buffer::{Quote, QuoteBuffer},
    codegen::CodeUnit,
    error::TaskError,
    provider::{Provider, TestProvider},
    runtime::{self, Cancellation, Limits},
};
use std::{rc::Rc, sync::Arc};
use tokio::sync::Notify;
const SOURCE: &str = include_str!("../fixtures/weighted_scan.py");
fn q(price: i64, quantity: i64) -> Quote {
    Quote {
        unit_price_cents: price,
        quantity,
    }
}
async fn execute(source: &str, rows: Vec<Quote>) -> (Result<i64, TaskError>, Arc<TestProvider>) {
    let p = Arc::new(TestProvider::new(rows, Default::default()));
    let result = runtime::execute(
        Rc::new(CodeUnit::compile(source).unwrap()),
        p.clone(),
        0,
        i64::MAX,
        Limits {
            chunk: 1,
            ..Limits::default()
        },
        Cancellation::default(),
        None,
    )
    .await;
    (result, p)
}
#[tokio::test(flavor = "current_thread")]
async fn checked_arithmetic_and_boolean_error_order_execute_native() {
    for (init, body, rows, expected) in [
        (
            "0",
            "total += q.unit_price_cents * q.quantity",
            vec![q(i64::MAX, 1), q(1, 1)],
            Err(TaskError::Overflow),
        ),
        (
            "-9223372036854775808",
            "total -= q.quantity",
            vec![q(1, 1)],
            Err(TaskError::Overflow),
        ),
        (
            "9223372036854775807",
            "total *= q.quantity",
            vec![q(1, 2)],
            Err(TaskError::Overflow),
        ),
        (
            "9223372036854775807 + 1",
            "total += 0",
            vec![],
            Err(TaskError::Overflow),
        ),
        ("0", "total -= -q.quantity", vec![q(-10, 3)], Ok(3)),
    ] {
        let s = SOURCE
            .replace("total: i64 = 0", &format!("total: i64 = {init}"))
            .replace("total += q.unit_price_cents * q.quantity", body);
        let (r, p) = execute(&s, rows).await;
        assert_eq!(r, expected);
        if r.is_err() {
            assert_eq!(p.record_count(), 0);
        }
    }
    for condition in [
        "False and (9223372036854775807 + 1 > 0)",
        "True or (9223372036854775807 + 1 > 0)",
    ] {
        let s = SOURCE.replace(
            "q.quantity > 0 and q.unit_price_cents <= price_limit",
            condition,
        );
        let (r, _) = execute(&s, vec![q(2, 3)]).await;
        assert_eq!(r, Ok(if condition.starts_with("False") { 0 } else { 6 }));
    }
    let s = SOURCE.replace(
        "record_total(batch_id, total)",
        "record_total(batch_id, total + 9223372036854775807)",
    );
    let (r, p) = execute(&s, vec![q(1, 1)]).await;
    assert_eq!(r, Err(TaskError::Overflow));
    assert_eq!(p.record_count(), 0);
}
#[tokio::test(flavor = "current_thread")]
async fn stateful_source_changes_code_and_chunk_resume_result() {
    let s = include_str!("../fixtures/stateful_branch.py");
    let a = CodeUnit::compile(SOURCE).unwrap();
    let b = CodeUnit::compile(s).unwrap();
    assert_ne!(a.clif, b.clif);
    let (r, _) = execute(s, vec![q(10, 2), q(8, 3), q(11, 1)]).await;
    assert_eq!(r, Ok(31));
}
#[tokio::test(flavor = "current_thread")]
async fn malformed_limits_buffer_caps_and_provider_errors_are_task_errors() {
    for (rows, chunk, bytes) in [(0, 1, 16), (10, 0, 16), (10, 4097, 16), (10, 1, 15)] {
        let p = Arc::new(TestProvider::new(vec![q(1, 1)], Default::default()));
        let r = runtime::execute(
            Rc::new(CodeUnit::compile(SOURCE).unwrap()),
            p.clone(),
            0,
            100,
            Limits {
                rows,
                chunk,
                buffer_bytes: bytes,
                ..Limits::default()
            },
            Cancellation::default(),
            None,
        )
        .await;
        assert_eq!(r, Err(TaskError::Budget));
        assert_eq!(p.record_count(), 0);
    }
    for (load, record) in [(true, false), (false, true)] {
        let p = Arc::new(
            TestProvider::new(vec![q(1, 1)], Default::default()).with_failures(load, record),
        );
        let r = runtime::execute(
            Rc::new(CodeUnit::compile(SOURCE).unwrap()),
            p.clone(),
            0,
            100,
            Limits::default(),
            Cancellation::default(),
            None,
        )
        .await;
        assert!(matches!(r, Err(TaskError::Provider(_))));
        assert_eq!(p.record_count(), usize::from(record));
        if let Some(weak) = p.last_rows() {
            assert!(weak.upgrade().is_none());
        }
    }
}
struct LateLoad {
    started: Notify,
    finish: Notify,
    completed: Notify,
    buffer: QuoteBuffer,
}
impl Provider for LateLoad {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        self.started.notify_one();
        self.finish.notified().await;
        self.completed.notify_one();
        Ok(self.buffer.clone())
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        panic!("cancelled load must not record")
    }
}
#[tokio::test(flavor = "current_thread")]
async fn cancellation_returns_before_provider_and_detached_owner_survives() {
    let provider = Arc::new(LateLoad {
        started: Notify::new(),
        finish: Notify::new(),
        completed: Notify::new(),
        buffer: QuoteBuffer::from_quotes(vec![q(3, 2)]).unwrap(),
    });
    let provider_weak = Arc::downgrade(&provider);
    let rows_weak = provider.buffer.downgrade();
    let code = Rc::new(CodeUnit::compile(SOURCE).unwrap());
    let code_weak = Rc::downgrade(&code);
    let cancel = Cancellation::default();
    let mut task = Box::pin(runtime::execute(
        code,
        provider.clone(),
        0,
        10,
        Limits::default(),
        cancel.clone(),
        None,
    ));
    tokio::select! { biased; _ = &mut task => panic!("load must suspend"), _ = provider.started.notified() => {} }
    cancel.cancel();
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut task)
            .await
            .unwrap(),
        Err(TaskError::Cancelled)
    );
    drop(task);
    assert!(code_weak.upgrade().is_none());
    assert!(
        rows_weak.upgrade().is_some(),
        "provider owns its live buffer after task cancellation"
    );
    provider.finish.notify_one();
    provider.completed.notified().await;
    drop(provider);
    tokio::task::yield_now().await;
    assert!(provider_weak.upgrade().is_none());
    assert!(rows_weak.upgrade().is_none());
}
#[tokio::test(flavor = "current_thread")]
async fn dropping_receiver_during_load_also_releases_late_buffer() {
    let provider = Arc::new(LateLoad {
        started: Notify::new(),
        finish: Notify::new(),
        completed: Notify::new(),
        buffer: QuoteBuffer::from_quotes(vec![q(3, 2)]).unwrap(),
    });
    let weak = provider.buffer.downgrade();
    let mut task = Box::pin(runtime::execute(
        Rc::new(CodeUnit::compile(SOURCE).unwrap()),
        provider.clone(),
        0,
        10,
        Limits::default(),
        Cancellation::default(),
        None,
    ));
    tokio::select! { biased; _ = &mut task => panic!("load must suspend"), _ = provider.started.notified() => {} }
    drop(task);
    provider.finish.notify_one();
    provider.completed.notified().await;
    drop(provider);
    tokio::task::yield_now().await;
    assert!(weak.upgrade().is_none());
}
#[tokio::test(flavor = "current_thread")]
async fn cancellation_at_chunk_boundary_prevents_record_and_drops_rows() {
    let p = Arc::new(TestProvider::new(vec![q(1, 1); 100], Default::default()));
    let trace: runtime::Trace = Rc::default();
    let cancel = Cancellation::default();
    let task = runtime::execute(
        Rc::new(CodeUnit::compile(SOURCE).unwrap()),
        p.clone(),
        0,
        10,
        Limits {
            chunk: 1,
            ..Limits::default()
        },
        cancel.clone(),
        Some(trace.clone()),
    );
    let canceller = async {
        loop {
            if trace.borrow().iter().any(|e| e.phase == "yield") {
                cancel.cancel();
                break;
            }
            tokio::task::yield_now().await;
        }
    };
    let (r, ()) = tokio::join!(task, canceller);
    assert_eq!(r, Err(TaskError::Cancelled));
    assert_eq!(p.record_count(), 0);
    assert!(p.last_rows().unwrap().upgrade().is_none());
}
// Preserve a pending-record test with no output before cancellation, rather than only racing a completed receipt.
struct LateRecord {
    started: Notify,
    finish: Notify,
}
impl Provider for LateRecord {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        QuoteBuffer::from_quotes(vec![])
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        self.started.notify_one();
        self.finish.notified().await;
        Ok(1)
    }
}
#[tokio::test(flavor = "current_thread")]
async fn cancelled_record_does_not_pin_code_until_io_completion() {
    let p = Arc::new(LateRecord {
        started: Notify::new(),
        finish: Notify::new(),
    });
    let c = Rc::new(CodeUnit::compile(SOURCE).unwrap());
    let weak = Rc::downgrade(&c);
    let cancel = Cancellation::default();
    let mut task = Box::pin(runtime::execute(
        c,
        p.clone(),
        0,
        10,
        Limits::default(),
        cancel.clone(),
        None,
    ));
    tokio::select! { biased; _=&mut task=>panic!("record must suspend"), _=p.started.notified()=>{} }
    assert!(
        weak.upgrade().is_none(),
        "native code's final use precedes record I/O"
    );
    cancel.cancel();
    assert_eq!(task.await, Err(TaskError::Cancelled));
    assert!(weak.upgrade().is_none());
    p.finish.notify_one();
    tokio::task::yield_now().await;
}

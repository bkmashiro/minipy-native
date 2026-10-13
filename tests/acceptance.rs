use minipy_native::{
    buffer::{Quote, QuoteBuffer},
    codegen::CodeUnit,
    error::TaskError,
    provider::{Provider, TestProvider},
    runtime::{self, Cancellation, Limits, Trace},
};
use std::{
    rc::Rc,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::{Notify, oneshot};

const SOURCE: &str = r#"async def main(batch_id: i64, price_limit: i64) -> i64:
    quotes = await tools.load_quotes(batch_id)
    total: i64 = 0
    for q in quotes:
        if q.quantity > 0 and q.unit_price_cents <= price_limit:
            total += q.unit_price_cents * q.quantity
    return await tools.record_total(batch_id, total)
"#;

fn quote(unit_price_cents: i64, quantity: i64) -> Quote {
    Quote {
        unit_price_cents,
        quantity,
    }
}

fn code() -> Rc<CodeUnit> {
    Rc::new(CodeUnit::compile(SOURCE).expect("real source compiles to native code"))
}

fn limits() -> Limits {
    Limits {
        rows: 100,
        chunk: 8,
        buffer_bytes: 4096,
        allow_load: true,
        allow_record: true,
    }
}

async fn run(rows: Vec<Quote>, batch: i64, limit: i64) -> Result<i64, TaskError> {
    let provider = Arc::new(TestProvider::new(rows, Default::default()));
    runtime::execute(
        code(),
        provider,
        batch,
        limit,
        limits(),
        Cancellation::default(),
        None,
    )
    .await
}

#[tokio::test(flavor = "current_thread")]
async fn empty_filter_and_mixed_records_execute_real_native_code() {
    assert_eq!(run(vec![], 7, 100).await.unwrap(), 7);
    assert_eq!(
        run(vec![quote(3, 0), quote(101, 4)], 5, 100).await.unwrap(),
        5
    );
    let rows = vec![quote(4, 3), quote(200, 2), quote(-2, 5), quote(7, 2)];
    assert_eq!(run(rows, 11, 100).await.unwrap(), 27); // 12 - 10 + 14 + receipt batch id
}

#[tokio::test(flavor = "current_thread")]
async fn overflow_is_a_task_error_and_skipped_rows_do_not_overflow() {
    let provider = Arc::new(TestProvider::new(
        vec![quote(i64::MAX, 2)],
        Default::default(),
    ));
    assert_eq!(
        runtime::execute(
            code(),
            provider.clone(),
            0,
            i64::MAX,
            limits(),
            Cancellation::default(),
            None
        )
        .await,
        Err(TaskError::Overflow)
    );
    assert_eq!(
        provider.record_count(),
        0,
        "overflow must prevent record dispatch"
    );

    // The product overflows, but both rows are filtered before multiplication.
    let filtered = vec![quote(i64::MAX, 0), quote(i64::MAX, i64::MAX)];
    assert_eq!(run(filtered, 9, -1).await.unwrap(), 9);
}

#[tokio::test(flavor = "current_thread")]
async fn chunk_yields_resume_and_total_row_budget_is_not_refilled() {
    let rows = vec![quote(1, 1); 7];
    let provider = Arc::new(TestProvider::new(rows.clone(), Default::default()));
    let trace: Trace = Rc::default();
    let mut lim = limits();
    lim.rows = 10;
    lim.chunk = 2;
    let result = runtime::execute(
        code(),
        provider,
        3,
        10,
        lim,
        Cancellation::default(),
        Some(trace.clone()),
    )
    .await
    .unwrap();
    assert_eq!(result, 10);
    {
        let events = trace.borrow();
        let yields: Vec<_> = events.iter().filter(|e| e.phase == "yield").collect();
        assert_eq!(yields.len(), 3);
        assert_eq!(
            yields.iter().map(|e| e.index).collect::<Vec<_>>(),
            [2, 4, 6]
        );
        assert_eq!(
            yields.iter().map(|e| e.remaining).collect::<Vec<_>>(),
            [8, 6, 4]
        );
    }

    let provider = Arc::new(TestProvider::new(rows, Default::default()));
    let mut too_small = limits();
    too_small.rows = 6;
    too_small.chunk = 2;
    assert_eq!(
        runtime::execute(
            code(),
            provider.clone(),
            3,
            10,
            too_small,
            Cancellation::default(),
            None
        )
        .await,
        Err(TaskError::Budget)
    );
    assert_eq!(provider.record_count(), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn load_and_record_permissions_are_independent_and_rc_code_state_is_per_task() {
    let compiled = code();
    let denied_load = Arc::new(TestProvider::new(vec![quote(2, 3)], Default::default()));
    let mut no_load = limits();
    no_load.allow_load = false;
    assert_eq!(
        runtime::execute(
            compiled.clone(),
            denied_load.clone(),
            4,
            10,
            no_load,
            Cancellation::default(),
            None
        )
        .await,
        Err(TaskError::Permission)
    );
    assert_eq!(denied_load.load_count(), 0);
    assert_eq!(denied_load.record_count(), 0);

    let denied_record = Arc::new(TestProvider::new(vec![quote(2, 3)], Default::default()));
    let mut no_record = limits();
    no_record.allow_record = false;
    assert_eq!(
        runtime::execute(
            compiled.clone(),
            denied_record.clone(),
            4,
            10,
            no_record,
            Cancellation::default(),
            None
        )
        .await,
        Err(TaskError::Permission)
    );
    assert_eq!(denied_record.load_count(), 1);
    assert_eq!(denied_record.record_count(), 0);

    let p1 = Arc::new(TestProvider::new(vec![quote(2, 3)], Default::default()));
    let p2 = Arc::new(TestProvider::new(
        vec![quote(7, 2), quote(1, 4)],
        Default::default(),
    ));
    let (a, b) = tokio::join!(
        runtime::execute(
            compiled.clone(),
            p1,
            10,
            100,
            limits(),
            Cancellation::default(),
            None
        ),
        runtime::execute(
            compiled,
            p2,
            20,
            100,
            limits(),
            Cancellation::default(),
            None
        ),
    );
    assert_eq!(a.unwrap(), 16);
    assert_eq!(b.unwrap(), 38);
}

/// Provider whose load/record completions are controlled by the test. It captures
/// only a Weak row reference, allowing the second-await lifetime contract to be checked.
struct GatedProvider {
    load_rx: Mutex<Option<oneshot::Receiver<Result<QuoteBuffer, TaskError>>>>,
    record_rx: Mutex<Option<oneshot::Receiver<Result<i64, TaskError>>>>,
    load_started: Notify,
    record_started: Notify,
    records: AtomicUsize,
    weak_rows: Mutex<Option<Weak<[Quote]>>>,
}
impl GatedProvider {
    fn new(
        load_rx: oneshot::Receiver<Result<QuoteBuffer, TaskError>>,
        record_rx: oneshot::Receiver<Result<i64, TaskError>>,
    ) -> Self {
        Self {
            load_rx: Mutex::new(Some(load_rx)),
            record_rx: Mutex::new(Some(record_rx)),
            load_started: Notify::new(),
            record_started: Notify::new(),
            records: AtomicUsize::new(0),
            weak_rows: Mutex::new(None),
        }
    }
    async fn wait_load(&self) {
        self.load_started.notified().await;
    }
    async fn wait_record(&self) {
        self.record_started.notified().await;
    }
}
impl Provider for GatedProvider {
    async fn load_quotes(&self, _batch_id: i64) -> Result<QuoteBuffer, TaskError> {
        self.load_started.notify_one();
        let rx = self.load_rx.lock().unwrap().take().expect("one load");
        let buffer = rx
            .await
            .map_err(|_| TaskError::Provider("load gate dropped".into()))??;
        *self.weak_rows.lock().unwrap() = Some(buffer.downgrade());
        Ok(buffer)
    }
    async fn record_total(&self, _batch_id: i64, _total: i64) -> Result<i64, TaskError> {
        self.records.fetch_add(1, Ordering::SeqCst);
        self.record_started.notify_one();
        let rx = self.record_rx.lock().unwrap().take().expect("one record");
        rx.await
            .map_err(|_| TaskError::Provider("record gate dropped".into()))?
    }
}

#[tokio::test(flavor = "current_thread")]
async fn second_await_does_not_retain_quote_owner() {
    let (load_tx, load_rx) = oneshot::channel();
    let (record_tx, record_rx) = oneshot::channel();
    let provider = Arc::new(GatedProvider::new(load_rx, record_rx));
    let task = runtime::execute(
        code(),
        provider.clone(),
        6,
        100,
        limits(),
        Cancellation::default(),
        None,
    );
    tokio::pin!(task);
    let completion = async {
        provider.wait_load().await;
        let buffer = QuoteBuffer::from_quotes(vec![quote(5, 2)]).unwrap();
        load_tx.send(Ok(buffer)).unwrap();
        provider.wait_record().await;
        let weak = provider.weak_rows.lock().unwrap().as_ref().unwrap().clone();
        assert!(
            weak.upgrade().is_none(),
            "quote owner must be released before record await"
        );
        record_tx.send(Ok(16)).unwrap();
    };
    let (result, ()) = tokio::join!(&mut task, completion);
    assert_eq!(result.unwrap(), 16);
}

#[tokio::test(flavor = "current_thread")]
async fn cancellation_drops_late_load_and_record_results_without_reviving_task() {
    // Cancel while load is outstanding; late buffer completion cannot proceed to record.
    let (load_tx, load_rx) = oneshot::channel();
    let (_record_tx, record_rx) = oneshot::channel();
    let provider = Arc::new(GatedProvider::new(load_rx, record_rx));
    let cancel = Cancellation::default();
    let task = runtime::execute(
        code(),
        provider.clone(),
        1,
        10,
        limits(),
        cancel.clone(),
        None,
    );
    tokio::pin!(task);
    let load_case = async {
        provider.wait_load().await;
        cancel.cancel();
        load_tx
            .send(Ok(QuoteBuffer::from_quotes(vec![quote(2, 3)]).unwrap()))
            .unwrap();
    };
    let (result, ()) = tokio::join!(&mut task, load_case);
    assert_eq!(result, Err(TaskError::Cancelled));
    assert_eq!(provider.records.load(Ordering::SeqCst), 0);

    // Cancel while record is outstanding; the late receipt is observed but rejected.
    let (load_tx, load_rx) = oneshot::channel();
    let (record_tx, record_rx) = oneshot::channel();
    let provider = Arc::new(GatedProvider::new(load_rx, record_rx));
    let cancel = Cancellation::default();
    let task = runtime::execute(
        code(),
        provider.clone(),
        1,
        10,
        limits(),
        cancel.clone(),
        None,
    );
    tokio::pin!(task);
    let record_case = async {
        provider.wait_load().await;
        load_tx
            .send(Ok(QuoteBuffer::from_quotes(vec![quote(2, 3)]).unwrap()))
            .unwrap();
        provider.wait_record().await;
        cancel.cancel();
        record_tx.send(Ok(7)).unwrap();
    };
    let (result, ()) = tokio::join!(&mut task, record_case);
    assert_eq!(result, Err(TaskError::Cancelled));
    assert_eq!(provider.records.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn provider_total_receipt_is_checked_for_overflow() {
    let provider = Arc::new(TestProvider::new(vec![quote(1, 1)], Default::default()));
    assert_eq!(
        runtime::execute(
            code(),
            provider,
            i64::MAX,
            1,
            limits(),
            Cancellation::default(),
            None
        )
        .await,
        Err(TaskError::Overflow)
    );
}

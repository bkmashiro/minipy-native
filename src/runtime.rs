//! Thread-local iterative task dispatcher with owned, detached I/O operations.
use crate::{
    buffer::QuoteBuffer,
    codegen::{CodeUnit, Frame},
    error::TaskError,
    provider::Provider,
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{Notify, oneshot};
#[derive(Default)]
struct CancelState {
    cancelled: AtomicBool,
    notify: Notify,
}
#[derive(Clone, Default)]
pub struct Cancellation(Arc<CancelState>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
        self.0.notify.notify_waiters();
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }
    async fn cancelled(&self) {
        let notification = self.0.notify.notified();
        tokio::pin!(notification);
        // Register before checking the flag so concurrent cancellation cannot be lost.
        notification.as_mut().enable();
        if !self.is_cancelled() {
            notification.await;
        }
    }
}
#[derive(Clone, Copy)]
pub struct Limits {
    pub rows: u64,
    pub chunk: usize,
    pub buffer_bytes: usize,
    pub allow_load: bool,
    pub allow_record: bool,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            rows: 1_000_000,
            chunk: 256,
            buffer_bytes: 16_000_000,
            allow_load: true,
            allow_record: true,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Event {
    pub phase: &'static str,
    pub buffer_bytes: usize,
    pub frame_bytes: usize,
    pub remaining: u64,
    pub index: u64,
}
pub type Trace = Rc<RefCell<Vec<Event>>>;
fn event(
    trace: &Option<Trace>,
    phase: &'static str,
    buffer_bytes: usize,
    frame_bytes: usize,
    remaining: u64,
    index: u64,
) {
    if let Some(t) = trace {
        t.borrow_mut().push(Event {
            phase,
            buffer_bytes,
            frame_bytes,
            remaining,
            index,
        });
    }
}
fn check(cancel: &Cancellation) -> Result<(), TaskError> {
    if cancel.is_cancelled() {
        Err(TaskError::Cancelled)
    } else {
        Ok(())
    }
}
/// Futures own all required arguments. Dropping the receiver does not drop the operation.
async fn receive<T>(
    rx: oneshot::Receiver<Result<T, TaskError>>,
    cancel: &Cancellation,
) -> Result<T, TaskError> {
    let result = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(TaskError::Cancelled),
        result = rx => result.map_err(|_| TaskError::Provider("operation lost".into()))?,
    };
    check(cancel)?;
    result
}
pub async fn execute<P: Provider>(
    code: Rc<CodeUnit>,
    provider: Arc<P>,
    batch: i64,
    price_limit: i64,
    limits: Limits,
    cancel: Cancellation,
    trace: Option<Trace>,
) -> Result<i64, TaskError> {
    check(&cancel)?;
    if !limits.allow_load {
        return Err(TaskError::Permission);
    }
    if limits.chunk == 0 || limits.chunk > 4096 {
        return Err(TaskError::Budget);
    }
    let mut remaining = limits.rows;
    event(&trace, "load_wait", 0, 0, remaining, 0);
    let (tx, rx) = oneshot::channel();
    let p = provider.clone();
    tokio::spawn(async move {
        let result = p.load_quotes(batch).await;
        let _ = tx.send(result);
    });
    let quotes: QuoteBuffer = receive(rx, &cancel).await?;
    if quotes.bytes() > limits.buffer_bytes {
        return Err(TaskError::Budget);
    }
    let mut frame = Box::new(Frame::new(batch, price_limit));
    let rows = quotes.rows();
    loop {
        check(&cancel)?;
        let left = rows.len() - (frame.index as usize);
        if left > 0 && remaining == 0 {
            return Err(TaskError::Budget);
        }
        let count = left
            .min(limits.chunk)
            .min(remaining.min(usize::MAX as u64) as usize);
        // Reserve bounded row work once. No resetting fuel on I/O or yield.
        remaining -= count as u64;
        let end = frame.index as usize + count;
        code.scan(&mut frame, rows, end)?;
        if end == rows.len() {
            break;
        }
        event(
            &trace,
            "yield",
            quotes.bytes(),
            std::mem::size_of::<Frame>(),
            remaining,
            frame.index,
        );
        tokio::task::yield_now().await;
    }
    let total = frame.total;
    let index = frame.index;
    // Scope has proven final buffer use. No quotes/continuation is sent to record.
    drop(frame);
    drop(quotes);
    // No remaining state can enter native code. A one-shot CodeUnit need not survive record I/O.
    drop(code);
    check(&cancel)?;
    if !limits.allow_record {
        return Err(TaskError::Permission);
    }
    event(&trace, "record_wait", 0, 0, remaining, index);
    let (tx, rx) = oneshot::channel();
    let p = provider.clone();
    tokio::spawn(async move {
        let result = p.record_total(batch, total).await;
        let _ = tx.send(result);
    });
    let receipt = receive(rx, &cancel).await?;
    event(&trace, "done", 0, 0, remaining, index);
    Ok(receipt)
}

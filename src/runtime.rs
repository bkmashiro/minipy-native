//! Thread-local iterative task dispatcher with owned, detached I/O operations.
use crate::{
    buffer::QuoteBuffer,
    codegen::{CodeUnit, Frame, View},
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
    if !code.legacy {
        return Err(TaskError::Input);
    }
    let mut output = Output::new(0, false);
    run(
        code,
        provider,
        &[batch, price_limit],
        ScriptLimits {
            limits,
            ..ScriptLimits::default()
        },
        cancel,
        trace,
        &mut output,
    )
    .await
}

/// Additional script bounds. A Tool invocation does not reset scan fuel.
#[derive(Clone, Copy)]
pub struct ScriptLimits {
    pub limits: Limits,
    pub tool_calls: u64,
    pub stdout_bytes: usize,
    pub allow_print: bool,
}
impl Default for ScriptLimits {
    fn default() -> Self {
        Self {
            limits: Limits::default(),
            tool_calls: 512,
            stdout_bytes: 16 * 1024,
            allow_print: true,
        }
    }
}
pub struct ScriptOutcome {
    pub status: Result<(), TaskError>,
    pub stdout: String,
}
pub(crate) struct Output {
    stdout: String,
    max: usize,
    allowed: bool,
    error: Option<TaskError>,
}
impl Output {
    fn new(max: usize, allowed: bool) -> Self {
        Self {
            stdout: String::new(),
            max,
            allowed,
            error: None,
        }
    }
}
/// Fixed native ABI, task-private output only. No I/O, callback dispatch or panic
/// on user data. One atomic line append: a rejected line leaves earlier output intact.
pub(crate) unsafe extern "C" fn native_print(output: *mut Output, value: i64, boolean: i64) -> i64 {
    // Only generated code calls this with its dispatcher-owned live Output.
    let output = unsafe { &mut *output };
    if !output.allowed {
        output.error = Some(TaskError::Permission);
        return 1;
    }
    let mut bytes = [0u8; 21];
    bytes[20] = b'\n';
    let line = if boolean != 0 {
        if value == 0 { "False\n" } else { "True\n" }
    } else {
        let mut n = value.unsigned_abs();
        let mut position = 20;
        loop {
            position -= 1;
            bytes[position] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        if value < 0 {
            position -= 1;
            bytes[position] = b'-';
        }
        // The stack buffer contains only ASCII digits, minus and newline.
        unsafe { std::str::from_utf8_unchecked(&bytes[position..]) }
    };
    if line.len() > output.max.saturating_sub(output.stdout.len()) {
        output.error = Some(TaskError::Budget);
        return 1;
    }
    if output.stdout.try_reserve_exact(line.len()).is_err() {
        output.error = Some(TaskError::Budget);
        return 1;
    }
    output.stdout.push_str(line);
    0
}
pub async fn execute_script<P: Provider>(
    code: Rc<CodeUnit>,
    provider: Arc<P>,
    inputs: &[i64],
    limits: ScriptLimits,
    cancel: Cancellation,
    trace: Option<Trace>,
) -> ScriptOutcome {
    let mut output = Output::new(limits.stdout_bytes, limits.allow_print);
    let status = run(code, provider, inputs, limits, cancel, trace, &mut output)
        .await
        .map(|_| ());
    ScriptOutcome {
        status,
        stdout: output.stdout,
    }
}
#[allow(clippy::too_many_arguments)]
async fn run<P: Provider>(
    code: Rc<CodeUnit>,
    provider: Arc<P>,
    inputs: &[i64],
    options: ScriptLimits,
    cancel: Cancellation,
    trace: Option<Trace>,
    output: &mut Output,
) -> Result<i64, TaskError> {
    check(&cancel)?;
    if inputs.len() != code.inputs.len()
        || inputs
            .iter()
            .zip(&code.inputs)
            .any(|(v, ty)| *ty == crate::ir::Type::Bool && ![0, 1].contains(v))
    {
        return Err(TaskError::Input);
    }
    let limits = options.limits;
    if limits.chunk == 0 || limits.chunk > 4096 {
        return Err(TaskError::Budget);
    }
    let mut calls = options.tool_calls;
    let mut owners: Vec<Option<QuoteBuffer>> = (0..code.buffers).map(|_| None).collect();
    let mut views = vec![View::default(); code.buffers];
    let mut frame = Box::new(Frame::new(inputs, limits.rows, limits.chunk));
    loop {
        check(&cancel)?;
        let status = code.resume(&mut frame, &views, output);
        if matches!(status, 0 | 2 | 3 | 4 | 7) {
            for (i, owner) in owners.iter_mut().enumerate() {
                if frame.live & (1u64 << i) == 0 {
                    *owner = None;
                    views[i] = View::default();
                }
            }
        }
        let held: usize = owners.iter().flatten().map(QuoteBuffer::bytes).sum();
        match status {
            0 => {
                event(&trace, "done", 0, 0, frame.remaining, frame.index);
                return Ok(frame.total);
            }
            1 => return Err(TaskError::Overflow),
            5 => return Err(TaskError::Budget),
            6 => return Err(output.error.take().unwrap_or(TaskError::Budget)),
            4 => {
                event(
                    &trace,
                    "yield",
                    held,
                    std::mem::size_of::<Frame>(),
                    frame.remaining,
                    frame.index,
                );
                tokio::task::yield_now().await;
            }
            2 | 3 | 7 => {
                check(&cancel)?;
                if (status == 2 && !limits.allow_load) || (status != 2 && !limits.allow_record) {
                    return Err(TaskError::Permission);
                }
                if calls == 0 {
                    return Err(TaskError::Budget);
                }
                calls -= 1;
                let batch = frame.batch;
                let total = frame.total;
                let target = frame.target as usize;
                if status == 7 {
                    let remaining = frame.remaining;
                    let index = frame.index;
                    // Proven terminal record: no remaining native continuation.
                    drop(frame);
                    drop(owners);
                    drop(views);
                    drop(code);
                    event(&trace, "record_wait", 0, 0, remaining, index);
                    let (tx, rx) = oneshot::channel();
                    tokio::spawn(async move {
                        let _ = tx.send(provider.record_total(batch, total).await);
                    });
                    let receipt = receive(rx, &cancel).await?;
                    event(&trace, "done", 0, 0, remaining, index);
                    return Ok(receipt);
                }
                let p = provider.clone();
                if status == 2 {
                    event(
                        &trace,
                        "load_wait",
                        held,
                        std::mem::size_of::<Frame>(),
                        frame.remaining,
                        frame.index,
                    );
                    let (tx, rx) = oneshot::channel();
                    tokio::spawn(async move {
                        let _ = tx.send(p.load_quotes(batch).await);
                    });
                    let quotes = receive(rx, &cancel).await?;
                    if quotes.bytes() > limits.buffer_bytes.saturating_sub(held) {
                        return Err(TaskError::Budget);
                    }
                    if frame.live & (1u64 << target) != 0 {
                        views[target] = View {
                            ptr: quotes.rows().as_ptr(),
                            len: quotes.rows().len() as u64,
                        };
                        owners[target] = Some(quotes);
                    }
                } else {
                    event(
                        &trace,
                        "record_wait",
                        held,
                        std::mem::size_of::<Frame>(),
                        frame.remaining,
                        frame.index,
                    );
                    let (tx, rx) = oneshot::channel();
                    tokio::spawn(async move {
                        let _ = tx.send(p.record_total(batch, total).await);
                    });
                    frame.locals[target] = receive(rx, &cancel).await?;
                }
            }
            _ => return Err(TaskError::Budget),
        }
    }
}

//! L trusted, fixed-program AArch64 assembly reference. No user-code injection.
//! Host wrapper retains input/tool/output/row bounds, detached waits and yields.
use minipy_native::{
    buffer::Quote,
    error::TaskError,
    provider::Provider,
    runtime::{ScriptLimits, ScriptOutcome},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
// Benchmark-only copy: the production token's async wait is crate-private.
#[derive(Default)]
struct CancelState {
    cancelled: AtomicBool,
    notify: tokio::sync::Notify,
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
        notification.as_mut().enable();
        if !self.is_cancelled() {
            notification.await;
        }
    }
}

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
std::arch::global_asm!(include_str!("weighted_aarch64.S"));
#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
unsafe extern "C" {
    fn minipy_l_scan(
        rows: *const Quote,
        count: usize,
        total: *mut i64,
        previous: *mut i64,
        limit: i64,
        stateful: u64,
    ) -> u64;
    static minipy_l_scan_end: u8;
}

pub fn code_bytes() -> Option<usize> {
    #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
    {
        Some((&raw const minipy_l_scan_end as usize) - (minipy_l_scan as *const () as usize))
    }
    #[cfg(not(all(target_arch = "aarch64", target_os = "linux")))]
    {
        None
    }
}

pub fn kernel(
    rows: &[Quote],
    total: &mut i64,
    previous: &mut i64,
    limit: i64,
    stateful: bool,
) -> Result<(), TaskError> {
    const {
        assert!(std::mem::size_of::<Quote>() == 16);
    }
    #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
    {
        // The immutable slice proves the whole count*16 region is alive/readable;
        // out-pointers are distinct mutable i64s. Assembly uses caller-saved GPRs,
        // no stack or syscalls. An empty slice is never dereferenced.
        match unsafe {
            minipy_l_scan(
                rows.as_ptr(),
                rows.len(),
                total,
                previous,
                limit,
                u64::from(stateful),
            )
        } {
            0 => Ok(()),
            _ => Err(TaskError::Overflow),
        }
    }
    #[cfg(not(all(target_arch = "aarch64", target_os = "linux")))]
    {
        let _ = (rows, total, previous, limit, stateful);
        Err(TaskError::Input)
    }
}

pub fn rust_kernel(
    rows: &[Quote],
    total: &mut i64,
    previous: &mut i64,
    limit: i64,
    stateful: bool,
) -> Result<(), TaskError> {
    for q in rows {
        if q.quantity > 0
            && q.unit_price_cents <= limit
            && (!stateful || q.unit_price_cents >= *previous)
        {
            let amount = q
                .unit_price_cents
                .checked_mul(q.quantity)
                .ok_or(TaskError::Overflow)?;
            *total = total.checked_add(amount).ok_or(TaskError::Overflow)?;
            if stateful {
                *previous = q.unit_price_cents;
            }
        }
    }
    Ok(())
}

fn check(cancel: &Cancellation) -> Result<(), TaskError> {
    if cancel.is_cancelled() {
        Err(TaskError::Cancelled)
    } else {
        Ok(())
    }
}
fn authorize(cancel: &Cancellation, allowed: bool, calls: &mut u64) -> Result<(), TaskError> {
    check(cancel)?;
    if !allowed {
        return Err(TaskError::Permission);
    }
    if *calls == 0 {
        return Err(TaskError::Budget);
    }
    *calls -= 1;
    Ok(())
}
async fn receive<T>(
    rx: tokio::sync::oneshot::Receiver<Result<T, TaskError>>,
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
    stateful: bool,
    provider: Arc<P>,
    inputs: &[i64],
    options: ScriptLimits,
    cancel: Cancellation,
) -> ScriptOutcome {
    let mut stdout = String::new();
    let status = async {
        check(&cancel)?;
        if inputs.len() != 2 {
            return Err(TaskError::Input);
        }
        let limits = options.limits;
        if limits.chunk == 0 || limits.chunk > 4096 {
            return Err(TaskError::Budget);
        }
        let (batch, limit) = (inputs[0], inputs[1]);
        let mut calls = options.tool_calls;
        authorize(&cancel, limits.allow_load, &mut calls)?;
        let p = provider.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let _ = tx.send(p.load_quotes(batch).await);
        });
        let quotes = receive(rx, &cancel).await?;
        if quotes.bytes() > limits.buffer_bytes {
            return Err(TaskError::Budget);
        }
        let (mut total, mut previous, mut index, mut remaining) = (0, 0, 0, limits.rows);
        while index < quotes.rows().len() {
            if remaining == 0 {
                return Err(TaskError::Budget);
            }
            let count = (quotes.rows().len() - index)
                .min(limits.chunk)
                .min(remaining as usize);
            remaining -= count as u64;
            kernel(
                &quotes.rows()[index..index + count],
                &mut total,
                &mut previous,
                limit,
                stateful,
            )?;
            index += count;
            if index < quotes.rows().len() {
                tokio::task::yield_now().await;
                check(&cancel)?;
            }
        }
        drop(quotes);
        authorize(&cancel, limits.allow_record, &mut calls)?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let _ = tx.send(provider.record_total(batch, total).await);
        });
        let receipt = receive(rx, &cancel).await?;
        if !options.allow_print {
            return Err(TaskError::Permission);
        }
        // Same scalar line contract; this benchmark Host uses format! rather than
        // the crate-private Output formatter. The extra allocation is timed.
        let line = format!("{receipt}\n");
        if line.len() > options.stdout_bytes {
            return Err(TaskError::Budget);
        }
        stdout.push_str(&line);
        Ok(())
    }
    .await;
    ScriptOutcome { status, stdout }
}

#[cfg(all(test, target_arch = "aarch64", target_os = "linux"))]
mod tests {
    use super::*;
    fn q(price: i64, quantity: i64) -> Quote {
        Quote {
            unit_price_cents: price,
            quantity,
        }
    }
    #[test]
    fn differential_boundaries_and_chunks() {
        let cases = vec![
            vec![],
            vec![q(i64::MAX, 0)],
            vec![q(i64::MAX, 2)],
            vec![q(i64::MIN, 2)],
            vec![q(i64::MIN, 1)],
            vec![q(i64::MAX, 1), q(1, 1)],
            vec![q(9, 2), q(8, 2), q(10, 3), q(12, -1), q(2, 1)],
        ];
        for rows in cases
            .into_iter()
            .chain([super::super::common::rows(100000)])
        {
            for stateful in [false, true] {
                for limit in [700, i64::MAX, i64::MIN] {
                    for chunk in [1, 2, 256, 4096] {
                        let (mut a, mut b, mut pa, mut pb) = (0, 0, 0, 0);
                        for rows in rows.chunks(chunk) {
                            let ar = kernel(rows, &mut a, &mut pa, limit, stateful);
                            let br = rust_kernel(rows, &mut b, &mut pb, limit, stateful);
                            assert_eq!(ar, br);
                            if ar.is_err() {
                                break;
                            } // partial failing-chunk state is private
                            assert_eq!((a, pa), (b, pb));
                        }
                    }
                }
            }
        }
        assert!(code_bytes().unwrap() > 0);
    }
}

//! Asynchronous provider contract and deterministic test implementation.

use crate::buffer::{Quote, QuoteBuffer, QuoteWeak};
use crate::error::TaskError;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Capability-facing asynchronous operations for the quote vertical slice.
pub trait Provider: Send + Sync + 'static {
    /// The Host binds a task capability set in its Provider instance. Schema is not authority.
    #[cfg(feature = "tool-json")]
    fn allows_tool(&self, _id: u32) -> bool {
        false
    }
    #[cfg(feature = "tool-json")]
    fn call_tool(
        &self,
        _id: u32,
        _args: [i64; 2],
        _count: usize,
    ) -> impl Future<Output = Result<crate::tool::Reply, TaskError>> + Send {
        async { Err(TaskError::Permission) }
    }
    /// Unified bound dispatch; numeric providers retain their existing ABI.
    #[cfg(feature = "tool-json")]
    fn call_bound_tool(
        &self,
        id: u32,
        args: crate::tool::ToolArguments,
    ) -> impl Future<Output = Result<crate::tool::Reply, TaskError>> + Send {
        async move {
            match args {
                crate::tool::ToolArguments::Numeric(values, count) => {
                    self.call_tool(id, values, count).await
                }
                crate::tool::ToolArguments::String(_) => Err(TaskError::Permission),
            }
        }
    }
    fn load_quotes(
        &self,
        batch_id: i64,
    ) -> impl Future<Output = Result<QuoteBuffer, TaskError>> + Send;

    fn record_total(
        &self,
        batch_id: i64,
        total: i64,
    ) -> impl Future<Output = Result<i64, TaskError>> + Send;
}

#[derive(Default)]
struct Counters {
    loads: AtomicUsize,
    records: AtomicUsize,
}

/// Finite per-operation delays for deterministic async tests and benchmarks.
#[derive(Clone, Copy, Debug, Default)]
pub struct Delays {
    pub load: Duration,
    pub record: Duration,
}

/// Deterministic provider with independent result owners and weak diagnostics.
/// It retains source rows but never a strong reference to a returned buffer.
pub struct TestProvider {
    source_rows: Arc<[Quote]>,
    delays: Delays,
    fail_load: bool,
    fail_record: bool,
    counters: Arc<Counters>,
    last_rows: Arc<Mutex<Option<QuoteWeak>>>,
}

impl TestProvider {
    pub fn new(rows: Vec<Quote>, delays: Delays) -> Self {
        Self {
            source_rows: Arc::from(rows),
            delays,
            fail_load: false,
            fail_record: false,
            counters: Arc::new(Counters::default()),
            last_rows: Arc::new(Mutex::new(None)),
        }
    }

    /// Configure deterministic failures, primarily for lifecycle/error tests.
    pub fn with_failures(mut self, load: bool, record: bool) -> Self {
        self.fail_load = load;
        self.fail_record = record;
        self
    }

    pub fn load_count(&self) -> usize {
        self.counters.loads.load(Ordering::Acquire)
    }

    pub fn record_count(&self) -> usize {
        self.counters.records.load(Ordering::Acquire)
    }

    /// Weakly observe the latest loaded row allocation without extending its life.
    pub fn last_rows(&self) -> Option<QuoteWeak> {
        self.last_rows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl Provider for TestProvider {
    async fn load_quotes(&self, _batch_id: i64) -> Result<QuoteBuffer, TaskError> {
        self.counters.loads.fetch_add(1, Ordering::Relaxed);
        tokio::time::sleep(self.delays.load).await;
        if self.fail_load {
            return Err(TaskError::Provider("load_quotes failed".to_owned()));
        }
        // Clone provider-owned source data into a distinct per-request allocation.
        let buffer = QuoteBuffer::from_quotes(self.source_rows.as_ref().to_vec())?;
        *self.last_rows.lock().unwrap_or_else(|e| e.into_inner()) = Some(buffer.downgrade());
        Ok(buffer)
    }

    async fn record_total(&self, batch_id: i64, total: i64) -> Result<i64, TaskError> {
        self.counters.records.fetch_add(1, Ordering::Relaxed);
        tokio::time::sleep(self.delays.record).await;
        if self.fail_record {
            return Err(TaskError::Provider("record_total failed".to_owned()));
        }
        total.checked_add(batch_id).ok_or(TaskError::Overflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn quote(price: i64, quantity: i64) -> Quote {
        Quote {
            unit_price_cents: price,
            quantity,
        }
    }

    #[tokio::test]
    async fn load_suspends_and_each_call_gets_an_independent_owner() {
        let provider = TestProvider::new(
            vec![quote(10, 2)],
            Delays {
                load: Duration::from_millis(10),
                record: Duration::ZERO,
            },
        );
        let started = tokio::time::Instant::now();
        let first = provider.load_quotes(4).await.unwrap();
        assert!(started.elapsed() >= Duration::from_millis(8));
        let first_weak = provider.last_rows().unwrap();
        let first_rows = first_weak.upgrade().unwrap();
        assert_eq!(first.rows(), &[quote(10, 2)]);
        let second = provider.load_quotes(4).await.unwrap();
        let second_weak = provider.last_rows().unwrap();
        assert!(!Arc::ptr_eq(&first_rows, &second_weak.upgrade().unwrap()));
        assert_eq!(second.rows(), &[quote(10, 2)]);
        assert_eq!(provider.load_count(), 2);
        drop(first_rows);
        drop(first);
        drop(second);
        assert!(first_weak.upgrade().is_none());
        assert!(second_weak.upgrade().is_none());
    }

    #[tokio::test]
    async fn cloned_buffer_lease_keeps_rows_alive_then_releases_them() {
        let provider = TestProvider::new(vec![quote(-7, i64::MAX)], Delays::default());
        let buffer = provider.load_quotes(9).await.unwrap();
        let weak = provider.last_rows().unwrap();
        let clone = buffer.clone();
        drop(buffer);
        assert!(weak.upgrade().is_some());
        drop(clone);
        assert!(weak.upgrade().is_none());
        assert_eq!(provider.record_count(), 0);
    }

    #[tokio::test]
    async fn record_is_async_checked_and_failures_are_returned() {
        let provider = TestProvider::new(
            Vec::new(),
            Delays {
                load: Duration::ZERO,
                record: Duration::from_millis(10),
            },
        );
        let started = tokio::time::Instant::now();
        assert_eq!(provider.record_total(8, 34).await.unwrap(), 42);
        assert!(started.elapsed() >= Duration::from_millis(8));
        assert_eq!(provider.record_count(), 1);
        assert_eq!(
            provider.record_total(1, i64::MAX).await.unwrap_err(),
            TaskError::Overflow
        );

        let failing = TestProvider::new(Vec::new(), Delays::default()).with_failures(true, true);
        assert!(matches!(
            failing.load_quotes(1).await,
            Err(TaskError::Provider(_))
        ));
        assert!(matches!(
            failing.record_total(1, 1).await,
            Err(TaskError::Provider(_))
        ));
    }
}

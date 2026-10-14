//! One diagnostic execution; counts are work inventory, never time shares.
#[cfg(feature = "interpreter-profile")]
fn main() {
    use minipy_native::{
        buffer::{Quote, QuoteBuffer},
        error::TaskError,
        interpreter::{self, InterpreterUnit},
        interpreter_profile,
        ir::Type,
        provider::Provider,
        runtime::{Cancellation, ScriptLimits},
    };
    use std::{rc::Rc, sync::Arc};
    struct Ready(usize);
    impl Provider for Ready {
        async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
            QuoteBuffer::from_quotes(
                (0..self.0)
                    .map(|i| Quote {
                        unit_price_cents: ((i * 37) % 1000) as i64,
                        quantity: (i % 9) as i64 - 2,
                    })
                    .collect(),
            )
        }
        async fn record_total(&self, batch: i64, total: i64) -> Result<i64, TaskError> {
            total.checked_add(batch).ok_or(TaskError::Overflow)
        }
    }
    let case = std::env::args().nth(1).unwrap();
    let chunk: usize = std::env::args().nth(2).unwrap().parse().unwrap();
    let source = if case == "weighted" {
        include_str!("../fixtures/weighted_script.py")
    } else {
        "quotes=tools.load_quotes(batch_id)\ntotal=0\nprevious=0\nfor quote in quotes:\n    if quote.quantity > 0 and quote.unit_price_cents <= price_limit and quote.unit_price_cents >= previous:\n        amount=quote.unit_price_cents * quote.quantity\n        total += amount\n        previous=quote.unit_price_cents\nreceipt=tools.record_total(batch_id,total)\nprint(receipt)"
    };
    let unit = Rc::new(
        InterpreterUnit::prepare_script(
            source,
            &[("batch_id", Type::I64), ("price_limit", Type::I64)],
        )
        .unwrap(),
    );
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let mut options = ScriptLimits::default();
    options.limits.chunk = chunk;
    interpreter_profile::reset();
    let out = rt.block_on(interpreter::execute_script(
        unit,
        Arc::new(Ready(100000)),
        &[17, 700],
        options,
        Cancellation::default(),
        None,
    ));
    out.status.unwrap();
    println!(
        "{case} rows=100000 chunk={chunk} stdout={:?} {:?}",
        out.stdout,
        interpreter_profile::take()
    );
}
#[cfg(not(feature = "interpreter-profile"))]
fn main() {
    panic!("requires interpreter-profile");
}

//! Precompiled execution reference for three fixed benchmark programs only.
//! Not a parser, compiler, backend, or unknown-source execution API.
use crate::{
    error::TaskError,
    provider::Provider,
    runtime::{self, Cancellation, Output, ScriptLimits, ScriptOutcome, Trace},
};
use std::sync::Arc;
use tokio::sync::oneshot;

#[derive(Clone, Copy, Debug)]
pub enum Fixture {
    Scalar,
    Weighted,
    Stateful,
}

pub async fn execute<P: Provider>(
    fixture: Fixture,
    provider: Arc<P>,
    inputs: &[i64],
    options: ScriptLimits,
    cancel: Cancellation,
    trace: Option<Trace>,
) -> ScriptOutcome {
    let mut output = Output::new(options.stdout_bytes, options.allow_print);
    let status = run(
        fixture,
        provider,
        inputs,
        options,
        cancel,
        trace,
        &mut output,
    )
    .await;
    ScriptOutcome {
        status,
        stdout: output.stdout,
    }
}
#[allow(clippy::too_many_arguments)]
async fn run<P: Provider>(
    fixture: Fixture,
    provider: Arc<P>,
    inputs: &[i64],
    options: ScriptLimits,
    cancel: Cancellation,
    trace: Option<Trace>,
    output: &mut Output,
) -> Result<(), TaskError> {
    runtime::check(&cancel)?;
    if inputs.len() != 2 {
        return Err(TaskError::Input);
    }
    let limits = options.limits;
    if limits.chunk == 0 || limits.chunk > 4096 {
        return Err(TaskError::Budget);
    }
    let (batch, price_limit) = (inputs[0], inputs[1]);
    let mut remaining = limits.rows;
    let mut index = 0;
    // Frame size is deliberately not claimed equal to either engine's state.
    let frame_bytes = std::mem::size_of::<(i64, i64, i64, u64, usize)>();
    if matches!(fixture, Fixture::Scalar) {
        let value = batch
            .checked_mul(2)
            .and_then(|v| v.checked_add(price_limit))
            .ok_or(TaskError::Overflow)?;
        output.append(value, false)?;
        output.append(i64::from(value > 0), true)?;
    } else {
        let mut calls = options.tool_calls;
        runtime::authorize_tool(&cancel, limits.allow_load, &mut calls)?;
        runtime::event(&trace, "load_wait", 0, frame_bytes, remaining, 0);
        let p = provider.clone();
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let _ = tx.send(p.load_quotes(batch).await);
        });
        let quotes = runtime::receive(rx, &cancel).await?;
        if quotes.bytes() > limits.buffer_bytes {
            return Err(TaskError::Budget);
        }
        let mut total: i64 = 0;
        let mut previous = 0;
        while index < quotes.rows().len() {
            if remaining == 0 {
                return Err(TaskError::Budget);
            }
            let count = (quotes.rows().len() - index)
                .min(limits.chunk)
                .min(remaining as usize);
            remaining -= count as u64;
            for q in &quotes.rows()[index..index + count] {
                if q.quantity > 0
                    && q.unit_price_cents <= price_limit
                    && (!matches!(fixture, Fixture::Stateful) || q.unit_price_cents >= previous)
                {
                    let amount = q
                        .unit_price_cents
                        .checked_mul(q.quantity)
                        .ok_or(TaskError::Overflow)?;
                    total = total.checked_add(amount).ok_or(TaskError::Overflow)?;
                    if matches!(fixture, Fixture::Stateful) {
                        previous = q.unit_price_cents;
                    }
                }
            }
            index += count;
            if index < quotes.rows().len() {
                runtime::event(
                    &trace,
                    "yield",
                    quotes.bytes(),
                    frame_bytes,
                    remaining,
                    index as u64,
                );
                tokio::task::yield_now().await;
                runtime::check(&cancel)?;
            }
        }
        // Proven fixed program last use, before authorizing/submitting record.
        drop(quotes);
        runtime::authorize_tool(&cancel, limits.allow_record, &mut calls)?;
        runtime::event(
            &trace,
            "record_wait",
            0,
            frame_bytes,
            remaining,
            index as u64,
        );
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let _ = tx.send(provider.record_total(batch, total).await);
        });
        let receipt = runtime::receive(rx, &cancel).await?;
        output.append(receipt, false)?;
    }
    runtime::event(&trace, "done", 0, 0, remaining, index as u64);
    Ok(())
}

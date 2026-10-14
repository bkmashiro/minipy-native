#![cfg(feature = "fixture-reference")]
use minipy_native::{
    buffer::Quote,
    codegen::CodeUnit,
    error::TaskError,
    fixture_reference::{self, Fixture},
    interpreter::{self, InterpreterUnit},
    ir::Type,
    provider::{Delays, TestProvider},
    runtime::{self, Cancellation, ScriptLimits, ScriptOutcome, Trace},
};
use std::{cell::RefCell, rc::Rc, sync::Arc};
const INPUTS: &[(&str, Type)] = &[("batch_id", Type::I64), ("price_limit", Type::I64)];
const STATEFUL: &str = "quotes=tools.load_quotes(batch_id)\ntotal=0\nprevious=0\nfor quote in quotes:\n    if quote.quantity > 0 and quote.unit_price_cents <= price_limit and quote.unit_price_cents >= previous:\n        amount=quote.unit_price_cents * quote.quantity\n        total += amount\n        previous=quote.unit_price_cents\nreceipt=tools.record_total(batch_id,total)\nprint(receipt)";
fn source(f: Fixture) -> &'static str {
    match f {
        Fixture::Scalar => include_str!("../fixtures/scalar_script.py"),
        Fixture::Weighted => include_str!("../fixtures/weighted_script.py"),
        Fixture::Stateful => STATEFUL,
    }
}
async fn compare(
    f: Fixture,
    values: &[i64],
    rows: Vec<Quote>,
    options: ScriptLimits,
    cancelled: bool,
) {
    let mut results = Vec::new();
    for engine in 0..3 {
        let p = Arc::new(TestProvider::new(rows.clone(), Delays::default()));
        let trace: Trace = Rc::new(RefCell::new(Vec::new()));
        let cancel = Cancellation::default();
        if cancelled {
            cancel.cancel();
        }
        let out = match engine {
            0 => {
                runtime::execute_script(
                    Rc::new(CodeUnit::compile_script(source(f), INPUTS).unwrap()),
                    p.clone(),
                    values,
                    options,
                    cancel,
                    Some(trace.clone()),
                )
                .await
            }
            1 => {
                interpreter::execute_script(
                    Rc::new(InterpreterUnit::prepare_script(source(f), INPUTS).unwrap()),
                    p.clone(),
                    values,
                    options,
                    cancel,
                    Some(trace.clone()),
                )
                .await
            }
            _ => {
                fixture_reference::execute(
                    f,
                    p.clone(),
                    values,
                    options,
                    cancel,
                    Some(trace.clone()),
                )
                .await
            }
        };
        let events = trace
            .borrow()
            .iter()
            .map(|e| (e.phase, e.buffer_bytes, e.remaining, e.index))
            .collect::<Vec<_>>();
        results.push((
            out.status,
            out.stdout,
            p.load_count(),
            p.record_count(),
            events,
        ));
    }
    assert_eq!(results[0], results[1], "native/interpreter {f:?}");
    assert_eq!(results[0], results[2], "native/reference {f:?}");
}
#[tokio::test]
async fn fixed_program_semantics_errors_and_chunk_boundaries() {
    let rows = vec![
        Quote {
            unit_price_cents: -2,
            quantity: 3,
        },
        Quote {
            unit_price_cents: 700,
            quantity: 4,
        },
        Quote {
            unit_price_cents: i64::MAX,
            quantity: 0,
        },
        Quote {
            unit_price_cents: 600,
            quantity: 2,
        },
    ];
    for f in [Fixture::Scalar, Fixture::Weighted, Fixture::Stateful] {
        for chunk in [1, 2, 256, 4096] {
            for fuel in [0, 1, 3, 4, 5] {
                let mut o = ScriptLimits::default();
                o.limits.chunk = chunk;
                o.limits.rows = fuel;
                compare(f, &[17, 700], rows.clone(), o, false).await;
            }
        }
        for limit in [0, 1, 2, 3, 8, 16384] {
            let o = ScriptLimits {
                stdout_bytes: limit,
                ..ScriptLimits::default()
            };
            compare(f, &[17, 700], rows.clone(), o, false).await;
        }
        for calls in [0, 1, 2] {
            compare(
                f,
                &[17, 700],
                rows.clone(),
                ScriptLimits {
                    tool_calls: calls,
                    ..ScriptLimits::default()
                },
                false,
            )
            .await;
        }
        for deny in 0..3 {
            let mut o = ScriptLimits::default();
            match deny {
                0 => o.limits.allow_load = false,
                1 => o.limits.allow_record = false,
                _ => o.allow_print = false,
            }
            compare(f, &[17, 700], rows.clone(), o, false).await;
        }
        for chunk in [0, 4097] {
            let mut o = ScriptLimits::default();
            o.limits.chunk = chunk;
            compare(f, &[17, 700], rows.clone(), o, false).await;
        }
        let mut o = ScriptLimits::default();
        o.limits.buffer_bytes = 63;
        compare(f, &[17, 700], rows.clone(), o, false).await;
        compare(f, &[17, 700], Vec::new(), ScriptLimits::default(), false).await;
        compare(f, &[], rows.clone(), ScriptLimits::default(), false).await;
        compare(f, &[17, 700], rows.clone(), ScriptLimits::default(), true).await;
        compare(
            f,
            &[i64::MAX, 700],
            rows.clone(),
            ScriptLimits {
                allow_print: false,
                ..ScriptLimits::default()
            },
            false,
        )
        .await;
        for bad in [
            vec![Quote {
                unit_price_cents: i64::MAX,
                quantity: 2,
            }],
            vec![
                Quote {
                    unit_price_cents: i64::MAX,
                    quantity: 1
                };
                2
            ],
        ] {
            compare(f, &[17, i64::MAX], bad, ScriptLimits::default(), false).await;
        }
    }
}

// Synchronize cancellation with an observed real chunk Pending, never a sleep.
#[tokio::test]
async fn reference_cancellation_at_first_chunk_releases_owner_and_blocks_record() {
    use std::{
        future::{Future, poll_fn},
        task::Poll,
    };
    let p = Arc::new(TestProvider::new(
        vec![
            Quote {
                unit_price_cents: 1,
                quantity: 1
            };
            10
        ],
        Delays::default(),
    ));
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let cancel = Cancellation::default();
    let mut o = ScriptLimits::default();
    o.limits.chunk = 1;
    let mut future = Box::pin(fixture_reference::execute(
        Fixture::Weighted,
        p.clone(),
        &[17, 700],
        o,
        cancel.clone(),
        Some(trace.clone()),
    ));
    poll_fn(|cx| match future.as_mut().poll(cx) {
        Poll::Pending if trace.borrow().iter().any(|e| e.phase == "yield") => Poll::Ready(()),
        Poll::Pending => Poll::Pending,
        Poll::Ready(_) => panic!("must yield before completion"),
    })
    .await;
    cancel.cancel();
    let ScriptOutcome { status, stdout } = future.await;
    assert_eq!(status, Err(TaskError::Cancelled));
    assert!(stdout.is_empty());
    assert_eq!(p.record_count(), 0);
    assert!(p.last_rows().unwrap().upgrade().is_none());
    let events = trace.borrow();
    let yield_event = events.iter().find(|e| e.phase == "yield").unwrap();
    assert_eq!((yield_event.remaining, yield_event.index), (999999, 1));
}

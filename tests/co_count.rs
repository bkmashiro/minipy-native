//! Bounded count-loop contract regressions, valid on the original tape as well.
use minipy_native::{
    buffer::Quote,
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    ir::Type,
    provider::{Delays, TestProvider},
    runtime::{self, Cancellation, ScriptLimits, ScriptOutcome, Trace},
};
use std::{
    cell::RefCell,
    future::{Future, poll_fn},
    rc::Rc,
    sync::Arc,
    task::Poll,
};

#[cfg(feature = "tool-json")]
#[tokio::test]
async fn count_retains_live_json_across_chunk_yields() {
    use minipy_native::{buffer::QuoteBuffer, provider::Provider, tool::*};
    struct Host;
    impl Provider for Host {
        async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
            QuoteBuffer::from_quotes(vec![
                Quote {
                    unit_price_cents: 1,
                    quantity: 1
                };
                600
            ])
        }
        async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
            Err(TaskError::Permission)
        }
        fn allows_tool(&self, id: u32) -> bool {
            id == 101
        }
        async fn call_tool(&self, _: u32, _: [i64; 2], _: usize) -> Result<Reply, TaskError> {
            Ok(Reply::Json(bytes::Bytes::from_static(
                br#"{"message":"okay"}"#,
            )))
        }
    }
    let tools = ToolEnvironment::new(vec![ToolDeclaration {
        id: 101,
        name: "fetch".into(),
        arguments: vec![],
        result: ResultSchema::JsonTyped(vec![(vec!["message".into()], JsonLeaf::String)]),
    }])
    .unwrap();
    let source = "data=tools.fetch()\nrows=tools.load_quotes(1)\ncount=0\nfor row in rows:\n    if row.quantity>0:\n        count=count+1\nprint(count)\nprint(data['message'])";
    let mut limits = ScriptLimits::default();
    limits.limits.chunk = 7;
    for native in [false, true] {
        let out = if native {
            runtime::execute_script(
                Rc::new(CodeUnit::compile_with_environment(source, &[], &tools).unwrap()),
                Arc::new(Host),
                &[],
                limits,
                Cancellation::default(),
                None,
            )
            .await
        } else {
            interpreter::execute_script(
                Rc::new(InterpreterUnit::prepare_with_environment(source, &[], &tools).unwrap()),
                Arc::new(Host),
                &[],
                limits,
                Cancellation::default(),
                None,
            )
            .await
        };
        assert_eq!(out.status, Ok(()));
        assert_eq!(out.stdout, "600\nokay\n");
    }
}

async fn check(
    source: &str,
    values: &[i64],
    rows: Vec<Quote>,
    limits: ScriptLimits,
) -> ScriptOutcome {
    let schema = [("initial", Type::I64), ("threshold", Type::I64)];
    let mut outcomes = Vec::new();
    let mut traces = Vec::new();
    for native in [false, true] {
        let provider = Arc::new(TestProvider::new(rows.clone(), Delays::default()));
        let trace: Trace = Rc::new(RefCell::new(Vec::new()));
        let out = if native {
            runtime::execute_script(
                Rc::new(CodeUnit::compile_script(source, &schema).unwrap()),
                provider.clone(),
                values,
                limits,
                Cancellation::default(),
                Some(trace.clone()),
            )
            .await
        } else {
            interpreter::execute_script(
                Rc::new(InterpreterUnit::prepare_script(source, &schema).unwrap()),
                provider.clone(),
                values,
                limits,
                Cancellation::default(),
                Some(trace.clone()),
            )
            .await
        };
        traces.push(
            trace
                .borrow()
                .iter()
                .map(|e| (e.phase, e.index, e.remaining, e.buffer_bytes))
                .collect::<Vec<_>>(),
        );
        outcomes.push((out, provider.record_count()));
    }
    assert_eq!(outcomes[0].0.status, outcomes[1].0.status, "{source}");
    assert_eq!(outcomes[0].0.stdout, outcomes[1].0.stdout, "{source}");
    assert_eq!(outcomes[0].1, outcomes[1].1);
    assert_eq!(traces[0], traces[1], "{source}");
    outcomes.remove(0).0
}

#[tokio::test]
async fn count_fields_distributions_tails_and_chunk_fuel() {
    for field in ["unit_price_cents", "quantity"] {
        let source = format!(
            "print(17)\nrows=tools.load_quotes(1)\ncount=initial\nfor row in rows:\n    if row.{field}>threshold:\n        count=count+1\nprint(count)\nprint(threshold)\nprint(initial)"
        );
        for size in [0, 1, 6, 7, 8, 255, 256, 257, 600] {
            let rows: Vec<_> = (0..size)
                .map(|i| Quote {
                    unit_price_cents: i % 17 - 8,
                    quantity: i % 7 - 3,
                })
                .collect();
            for threshold in [i64::MIN, 0, i64::MAX] {
                let hits = rows
                    .iter()
                    .filter(|r| {
                        (if field == "quantity" {
                            r.quantity
                        } else {
                            r.unit_price_cents
                        }) > threshold
                    })
                    .count();
                for chunk in [1, 7, 256, 4096] {
                    let mut limits = ScriptLimits::default();
                    limits.limits.chunk = chunk;
                    let out = check(&source, &[-5, threshold], rows.clone(), limits).await;
                    assert_eq!(out.status, Ok(()));
                    assert_eq!(
                        out.stdout,
                        format!("17\n{}\n{threshold}\n-5\n", hits as i64 - 5)
                    );
                }
            }
        }
        for fuel in [0, 1, 6, 7, 8, 20] {
            let rows = vec![
                Quote {
                    unit_price_cents: 1,
                    quantity: 1
                };
                20
            ];
            let mut limits = ScriptLimits::default();
            limits.limits.chunk = 7;
            limits.limits.rows = fuel;
            let out = check(&source, &[0, 0], rows, limits).await;
            assert_eq!(
                out.status,
                if fuel < 20 {
                    Err(TaskError::Budget)
                } else {
                    Ok(())
                }
            );
            if fuel < 20 {
                assert_eq!(out.stdout, "17\n");
            }
        }
    }
}

#[tokio::test]
async fn count_overflow_and_ordered_effects() {
    let source = "print(17)\nrows=tools.load_quotes(1)\ncount=initial\nfor row in rows:\n    if row.quantity>threshold:\n        count=count+1\nprint(count)\ntools.record_total(0,count)";
    for initial in [i64::MAX, i64::MAX - 1, i64::MIN] {
        for threshold in [-1, 0, 1] {
            let rows = vec![
                Quote {
                    unit_price_cents: i64::MIN,
                    quantity: 1
                };
                3
            ];
            let mut limits = ScriptLimits::default();
            limits.limits.chunk = 2;
            let out = check(source, &[initial, threshold], rows, limits).await;
            let expected = if threshold < 1 {
                initial.checked_add(3)
            } else {
                Some(initial)
            };
            assert_eq!(out.status, expected.map(|_| ()).ok_or(TaskError::Overflow));
            if expected.is_none() {
                assert_eq!(out.stdout, "17\n");
            }
        }
    }
    let rows = vec![
        Quote {
            unit_price_cents: 1,
            quantity: 1
        };
        3
    ];
    let mut denied = ScriptLimits::default();
    denied.limits.allow_load = false;
    assert_eq!(
        check(source, &[0, 0], rows.clone(), denied).await.status,
        Err(TaskError::Permission)
    );
    let mut tight = ScriptLimits::default();
    tight.limits.buffer_bytes = 47;
    assert_eq!(
        check(source, &[0, 0], rows, tight).await.status,
        Err(TaskError::Budget)
    );
}

#[tokio::test]
async fn count_nonmatching_shapes_and_multiple_loops() {
    let prefix = "rows=tools.load_quotes(1)\ncount=initial\nlimit=threshold\n";
    for body in [
        "if row.quantity>count:\n        count=count+1",
        "if row.quantity>threshold:\n        count=count+2",
        "if row.quantity>threshold:\n        count=count+row.unit_price_cents",
        "if row.quantity>threshold:\n        count=count+1\n    else:\n        count=count-1",
        "if row.quantity>threshold and row.unit_price_cents>0:\n        count=count+1",
        "if row.quantity+1>threshold:\n        count=count+1",
        "if row.quantity>limit:\n        count=count+1\n    limit=limit+1",
    ] {
        let source =
            format!("{prefix}for row in rows:\n    {body}\nprint(count)\nprint(threshold)");
        let out = check(
            &source,
            &[0, 0],
            vec![
                Quote {
                    unit_price_cents: 2,
                    quantity: 3
                };
                5
            ],
            ScriptLimits::default(),
        )
        .await;
        assert_eq!(out.status, Ok(()));
    }
    let source = format!(
        "{prefix}for row in rows:\n    if row.quantity>0:\n        count=count+1\nprint(count)\nfor row in rows:\n    if row.unit_price_cents>threshold:\n        count=count+1\nprint(count)"
    );
    let mut limits = ScriptLimits::default();
    limits.limits.chunk = 3;
    let out = check(
        &source,
        &[10, 0],
        vec![
            Quote {
                unit_price_cents: -2,
                quantity: 3
            };
            7
        ],
        limits,
    )
    .await;
    assert_eq!(out.stdout, "17\n17\n");
    assert!(
        InterpreterUnit::prepare_script(
            "rows=tools.load_quotes(1)\nfor row in rows:\n    x=row.quantity\nprint(row.quantity)",
            &[]
        )
        .is_err()
    );
}

#[tokio::test]
async fn count_cancels_at_first_chunk_and_reuses_immutable_unit() {
    let source = "print(17)\nrows=tools.load_quotes(1)\ncount=0\nfor row in rows:\n    if row.quantity>0:\n        count=count+1\nprint(count)";
    let unit = Rc::new(InterpreterUnit::prepare_script(source, &[]).unwrap());
    let provider = Arc::new(TestProvider::new(
        vec![
            Quote {
                unit_price_cents: 0,
                quantity: 1
            };
            600
        ],
        Delays::default(),
    ));
    let cancel = Cancellation::default();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let mut limits = ScriptLimits::default();
    limits.limits.chunk = 7;
    let job = interpreter::execute_script(
        unit.clone(),
        provider.clone(),
        &[],
        limits,
        cancel.clone(),
        Some(trace.clone()),
    );
    tokio::pin!(job);
    poll_fn(|cx| {
        assert!(job.as_mut().poll(cx).is_pending());
        if trace.borrow().iter().any(|e| e.phase == "yield") {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await;
    let e = trace
        .borrow()
        .iter()
        .find(|e| e.phase == "yield")
        .unwrap()
        .clone();
    assert_eq!(
        (e.index, e.remaining, e.buffer_bytes),
        (7, limits.limits.rows - 7, 9600)
    );
    let weak = provider.last_rows().unwrap();
    assert!(weak.upgrade().is_some());
    cancel.cancel();
    let out = job.await;
    assert_eq!(out.status, Err(TaskError::Cancelled));
    assert_eq!(out.stdout, "17\n");
    assert!(weak.upgrade().is_none());
    let provider = Arc::new(TestProvider::new(
        vec![
            Quote {
                unit_price_cents: 0,
                quantity: 1
            };
            3
        ],
        Delays::default(),
    ));
    let out =
        interpreter::execute_script(unit, provider, &[], limits, Cancellation::default(), None)
            .await;
    assert_eq!(out.status, Ok(()));
    assert_eq!(out.stdout, "17\n3\n");
}

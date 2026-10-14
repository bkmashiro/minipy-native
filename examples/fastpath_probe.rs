//! Narrow old scalar/Quote regression probe, runnable unchanged at the pre-AH revision.
use minipy_native::{
    buffer::{Quote, QuoteBuffer},
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    ir::Type,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
};
use std::{rc::Rc, sync::Arc, time::Instant};
struct P(QuoteBuffer);
impl Provider for P {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        Ok(self.0.clone())
    }
    async fn record_total(&self, batch: i64, total: i64) -> Result<i64, TaskError> {
        total.checked_add(batch).ok_or(TaskError::Overflow)
    }
}
fn main() {
    let mut builder = tokio::runtime::Builder::new_current_thread();
    if std::env::var_os("MINIPY_IO_DRIVER").is_some() {
        builder.enable_all();
    } else {
        builder.enable_time();
    }
    let rt = builder.build().unwrap();
    let p = Arc::new(P(QuoteBuffer::from_quotes(
        (0..1000)
            .map(|i| Quote {
                unit_price_cents: i % 97,
                quantity: i % 5,
            })
            .collect(),
    )
    .unwrap()));
    let inputs = &[("batch_id", Type::I64), ("price_limit", Type::I64)];
    println!("workload,arm,lifecycle,iterations,total_ns,prepared_bytes,code_bytes,checksum");
    for (work, source) in [
        ("scalar", include_str!("../fixtures/scalar_script.py")),
        ("weighted", include_str!("../fixtures/weighted_script.py")),
        ("stateful", include_str!("../fixtures/common_stateful.py")),
    ] {
        for native in [false, true] {
            let nu = native.then(|| Rc::new(CodeUnit::compile_script(source, inputs).unwrap()));
            let tu = (!native)
                .then(|| Rc::new(InterpreterUnit::prepare_script(source, inputs).unwrap()));
            for life in ["source_cold", "prepared"] {
                let f = || {
                    rt.block_on(async {
                        let r = if native {
                            runtime::execute_script(
                                if life == "source_cold" {
                                    Rc::new(CodeUnit::compile_script(source, inputs).unwrap())
                                } else {
                                    nu.as_ref().unwrap().clone()
                                },
                                p.clone(),
                                &[7, 700],
                                ScriptLimits::default(),
                                Cancellation::default(),
                                None,
                            )
                            .await
                        } else {
                            interpreter::execute_script(
                                if life == "source_cold" {
                                    Rc::new(
                                        InterpreterUnit::prepare_script(source, inputs).unwrap(),
                                    )
                                } else {
                                    tu.as_ref().unwrap().clone()
                                },
                                p.clone(),
                                &[7, 700],
                                ScriptLimits::default(),
                                Cancellation::default(),
                                None,
                            )
                            .await
                        };
                        assert_eq!(r.status, Ok(()));
                        r.stdout.bytes().map(i64::from).sum::<i64>()
                    })
                };
                let expected = f();
                let n = if life == "source_cold" { 20 } else { 1000 };
                let start = Instant::now();
                let mut sum = 0;
                for _ in 0..n {
                    sum += std::hint::black_box(f());
                }
                let elapsed = start.elapsed().as_nanos();
                assert_eq!(sum, expected * n);
                println!(
                    "{work},{},{life},{n},{elapsed},{},{},{sum}",
                    if native { "native" } else { "tape" },
                    tu.as_ref().map_or(0, |u| u.prepared_bytes()),
                    nu.as_ref().map_or(0, |u| u.code_bytes)
                );
            }
        }
    }
}

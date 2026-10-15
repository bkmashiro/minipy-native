//! Private diagnostic only. No bypass, new schema API or production timers.
use super::*;
use crate::{interpreter::InterpreterUnit, tool::ToolEnvironment};
use std::{hint::black_box, time::Instant};

#[test]
#[ignore = "explicit serial release diagnosis under perf lock"]
fn schema_prepare_diagnosis() {
    let inputs = [("input_0", Type::I64), ("input_1", Type::I64)];
    let environment = ToolEnvironment::new(vec![]).unwrap();
    // A distinct source in every prepare; caller construction is not timed.
    let sources: Vec<_> = (0..256).map(|n| format!("print({n} + 1)")).collect();
    for round in 0..8 {
        for count in if round % 2 == 0 { [0, 2] } else { [2, 0] } {
            let schema = black_box(&inputs[..count]);
            let validation = || {
                let start = Instant::now();
                for _ in 0..4096 {
                    black_box(validate_inputs(black_box(schema))).unwrap();
                }
                start.elapsed().as_nanos()
            };
            let prepare = || {
                let start = Instant::now();
                for source in &sources {
                    drop(black_box(
                        InterpreterUnit::prepare_with_environment(
                            black_box(source),
                            schema,
                            &environment,
                        )
                        .unwrap(),
                    ));
                }
                start.elapsed().as_nanos()
            };
            let frontend = || {
                let start = Instant::now();
                for source in &sources {
                    drop(black_box(
                        parse_with_environment(black_box(source), schema, &environment).unwrap(),
                    ));
                }
                start.elapsed().as_nanos()
            };
            let (validate_ns, frontend_ns, prepare_ns) = if round % 2 == 0 {
                (validation(), frontend(), prepare())
            } else {
                let p = prepare();
                let f = frontend();
                (validation(), f, p)
            };
            println!(
                "SCHEMA {{\"round\":{round},\"inputs\":{count},\"validate_batch_ns\":{validate_ns},\"validate_reps\":4096,\"frontend_batch_ns\":{frontend_ns},\"prepare_batch_ns\":{prepare_ns},\"prepare_reps\":256}}"
            );
        }
    }
}

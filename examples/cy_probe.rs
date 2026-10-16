//! Paired route/direct fresh-source lifecycle diagnostic, not a workload benchmark.
#[path = "cy_route.rs"]
mod host;
use host::{Backend, RouteOutcome};
use minipy_native::{
    codegen::CodeUnit,
    interpreter::{self, InterpreterUnit},
    provider::{Delays, TestProvider},
    runtime::{self, Cancellation, ScriptLimits},
};
use std::{rc::Rc, sync::Arc, time::Instant};
fn main() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let provider = Arc::new(TestProvider::new(vec![], Delays::default()));
    println!("backend,arm,sample,ns");
    for sample in 0..33 {
        for backend in [Backend::Tape, Backend::Native] {
            let arms = if sample % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            };
            for routed in arms {
                let source = format!("n={sample}\nprint(n + 1)");
                let start = Instant::now();
                let out = if routed {
                    let RouteOutcome::Executed(out) = rt.block_on(host::route(
                        backend,
                        &source,
                        &[],
                        &[],
                        provider.clone(),
                        ScriptLimits::default(),
                        Cancellation::default(),
                    )) else {
                        panic!("expected execution")
                    };
                    out
                } else {
                    match backend {
                        Backend::Native => {
                            let unit = Rc::new(CodeUnit::compile_script(&source, &[]).unwrap());
                            rt.block_on(runtime::execute_script(
                                unit,
                                provider.clone(),
                                &[],
                                ScriptLimits::default(),
                                Cancellation::default(),
                                None,
                            ))
                        }
                        Backend::Tape => {
                            let unit =
                                Rc::new(InterpreterUnit::prepare_script(&source, &[]).unwrap());
                            rt.block_on(interpreter::execute_script(
                                unit,
                                provider.clone(),
                                &[],
                                ScriptLimits::default(),
                                Cancellation::default(),
                                None,
                            ))
                        }
                    }
                };
                assert_eq!(out.status, Ok(()));
                assert_eq!(out.stdout, format!("{}\n", sample + 1));
                drop(out);
                let ns = start.elapsed().as_nanos();
                println!(
                    "{backend:?},{},{sample},{ns}",
                    if routed { "route" } else { "direct" }
                );
            }
        }
    }
}

//! Host-owned routing, without an external executor or implicit retry.
use minipy_native::{
    codegen::CodeUnit,
    interpreter::{self, InterpreterUnit},
    ir::{AdmissionKind, CompileError, Type},
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits, ScriptOutcome},
};
use std::{rc::Rc, sync::Arc};

#[derive(Clone, Copy, Debug)]
pub enum Backend {
    Native,
    Tape,
}

pub enum RouteOutcome {
    /// The Host may choose a separately authorized executor. Nothing ran here.
    Unsupported(CompileError),
    Rejected(CompileError),
    /// Includes runtime failure and partial stdout. Never replay this script.
    Executed(ScriptOutcome),
}

/// Prepare the entire source before giving execution access to the provider.
/// No cache, source rewrite, exception retry or automatic external execution.
pub async fn route<P: Provider>(
    backend: Backend,
    source: &str,
    inputs: &[(&str, Type)],
    values: &[i64],
    provider: Arc<P>,
    limits: ScriptLimits,
    cancel: Cancellation,
) -> RouteOutcome {
    enum Prepared {
        Native(Rc<CodeUnit>),
        Tape(Rc<InterpreterUnit>),
    }
    let prepared = match backend {
        Backend::Native => {
            CodeUnit::compile_script(source, inputs).map(|unit| Prepared::Native(Rc::new(unit)))
        }
        Backend::Tape => InterpreterUnit::prepare_script(source, inputs)
            .map(|unit| Prepared::Tape(Rc::new(unit))),
    };
    let unit = match prepared {
        Ok(unit) => unit,
        Err(error) if error.kind == AdmissionKind::Unsupported => {
            return RouteOutcome::Unsupported(error);
        }
        Err(error) => return RouteOutcome::Rejected(error),
    };
    let outcome = match unit {
        Prepared::Native(unit) => {
            runtime::execute_script(unit, provider, values, limits, cancel, None).await
        }
        Prepared::Tape(unit) => {
            interpreter::execute_script(unit, provider, values, limits, cancel, None).await
        }
    };
    RouteOutcome::Executed(outcome)
}

#[allow(dead_code)] // Also imported by the integration test.
fn main() {
    use minipy_native::provider::{Delays, TestProvider};
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    for backend in [Backend::Tape, Backend::Native] {
        for source in ["print(7)", "tools.record_total(1, 2)\nx = [1, 2]"] {
            let provider = Arc::new(TestProvider::new(vec![], Delays::default()));
            let outcome = rt.block_on(route(
                backend,
                source,
                &[],
                &[],
                provider.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            ));
            match outcome {
                RouteOutcome::Executed(out) => {
                    println!("{backend:?}: {:?}, stdout={:?}", out.status, out.stdout);
                }
                RouteOutcome::Unsupported(error) => {
                    println!("{backend:?}: Host routing stub only: {error}");
                }
                RouteOutcome::Rejected(error) => println!("{backend:?}: rejected: {error}"),
            }
            assert_eq!(provider.record_count(), 0);
        }
    }
}

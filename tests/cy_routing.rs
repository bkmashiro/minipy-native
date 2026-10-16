#![cfg(not(target_arch = "wasm32"))]
#[path = "../examples/cy_route.rs"]
mod host;
use host::{Backend, RouteOutcome, route};
use minipy_native::{
    error::TaskError,
    ir::{AdmissionKind, MAX_SOURCE_BYTES, Type},
    provider::{Delays, TestProvider},
    runtime::{Cancellation, ScriptLimits},
};
use std::sync::Arc;

// This stub only records the Host's routing decision. It never executes Python.
fn choose(outcome: &RouteOutcome, fallback: &mut usize) {
    if matches!(outcome, RouteOutcome::Unsupported(_)) {
        *fallback += 1;
    }
}
fn provider(fail: bool) -> Arc<TestProvider> {
    Arc::new(TestProvider::new(vec![], Delays::default()).with_failures(false, fail))
}

#[tokio::test]
async fn fresh_source_success_and_whole_script_admission_before_any_effect() {
    for backend in [Backend::Tape, Backend::Native] {
        let mut fallback = 0;
        for value in [7, 8] {
            let p = provider(false);
            let source = format!("print(1)\nr=tools.record_total(2, {value})\nprint(r)");
            let out = route(
                backend,
                &source,
                &[],
                &[],
                p.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            choose(&out, &mut fallback);
            let RouteOutcome::Executed(out) = out else {
                panic!("expected execution")
            };
            assert_eq!(out.status, Ok(()));
            assert_eq!(out.stdout, format!("1\n{}\n", value + 2));
            assert_eq!(p.record_count(), 1);
        }
        assert_eq!(fallback, 0);
        // A naive execute-until-unsupported router would already have printed/called.
        for suffix in [
            "x = [1, 2]",
            "def f():\n    return 1",
            "if False:\n    x = [1]",
        ] {
            let source = format!("print(1)\ntools.record_total(2, 3)\n{suffix}");
            let p = provider(false);
            let out = route(
                backend,
                &source,
                &[],
                &[],
                p.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            choose(&out, &mut fallback);
            let RouteOutcome::Unsupported(error) = out else {
                panic!("expected routing request")
            };
            assert_eq!(error.kind, AdmissionKind::Unsupported);
            assert!(error.location.is_some());
            assert_eq!((p.load_count(), p.record_count()), (0, 0));
        }
        assert_eq!(fallback, 3);
    }
}

#[tokio::test]
async fn malformed_type_schema_and_preparation_limits_are_rejected_not_routed() {
    let oversized = "#".repeat(MAX_SOURCE_BYTES + 1);
    let deep = format!("print({}1{})", "(".repeat(33), ")".repeat(33));
    let locals = (0..33).map(|n| format!("v{n}={n}\n")).collect::<String>();
    for backend in [Backend::Tape, Backend::Native] {
        for (source, inputs, kind) in [
            (
                "print(1)\ntools.record_total(2,3)\nif :",
                &[][..],
                AdmissionKind::Syntax,
            ),
            ("print(1 + True)", &[][..], AdmissionKind::Type),
            (
                "print(1)",
                &[("print", Type::I64)][..],
                AdmissionKind::Schema,
            ),
            (oversized.as_str(), &[][..], AdmissionKind::Resource),
            (deep.as_str(), &[][..], AdmissionKind::Resource),
            (locals.as_str(), &[][..], AdmissionKind::Resource),
        ] {
            let p = provider(false);
            let out = route(
                backend,
                source,
                inputs,
                &[],
                p.clone(),
                ScriptLimits::default(),
                Cancellation::default(),
            )
            .await;
            let mut fallback = 0;
            choose(&out, &mut fallback);
            let RouteOutcome::Rejected(error) = out else {
                panic!("expected rejection: {kind:?}")
            };
            assert_eq!(error.kind, kind);
            assert_eq!(fallback, 0);
            assert_eq!((p.load_count(), p.record_count()), (0, 0));
        }
    }
}

#[tokio::test]
async fn runtime_errors_keep_partial_stdout_and_never_replay_or_fallback() {
    for backend in [Backend::Tape, Backend::Native] {
        // First call succeeds before each later failure except provider failure.
        for case in 0..6 {
            let p = provider(case == 2);
            let mut limits = ScriptLimits::default();
            let cancel = Cancellation::default();
            let (source, expected, stdout, calls) = match case {
                0 => (
                    "print(1)\ntools.record_total(2,3)\nprint(9223372036854775807 + 1)\ntools.record_total(2,4)",
                    TaskError::Overflow,
                    "1\n",
                    1,
                ),
                1 => {
                    limits.limits.allow_record = false;
                    (
                        "print(1)\ntools.record_total(2,3)",
                        TaskError::Permission,
                        "1\n",
                        0,
                    )
                }
                2 => (
                    "print(1)\ntools.record_total(2,3)\ntools.record_total(2,4)",
                    TaskError::Provider("record_total failed".into()),
                    "1\n",
                    1,
                ),
                3 => {
                    limits.tool_calls = 1;
                    (
                        "print(1)\ntools.record_total(2,3)\ntools.record_total(2,4)",
                        TaskError::Budget,
                        "1\n",
                        1,
                    )
                }
                4 => {
                    cancel.cancel();
                    (
                        "print(1)\ntools.record_total(2,3)",
                        TaskError::Cancelled,
                        "",
                        0,
                    )
                }
                _ => ("print(1)\ntools.record_total(2,3)", TaskError::Input, "", 0),
            };
            let inputs = if case == 5 {
                &[("n", Type::I64)][..]
            } else {
                &[][..]
            };
            let out = route(backend, source, inputs, &[], p.clone(), limits, cancel).await;
            let mut fallback = 0;
            choose(&out, &mut fallback);
            let RouteOutcome::Executed(out) = out else {
                panic!("runtime error routed")
            };
            assert_eq!(out.status, Err(expected), "{backend:?} case {case}");
            assert_eq!(out.stdout, stdout);
            assert_eq!(p.record_count(), calls);
            assert_eq!(fallback, 0);
        }
    }
}

#[tokio::test]
async fn cancellation_after_submission_keeps_effect_once_and_never_routes() {
    use minipy_native::{buffer::QuoteBuffer, provider::Provider};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Notify;
    #[derive(Default)]
    struct Gate {
        calls: AtomicUsize,
        entered: Notify,
        release: Notify,
        completed: Notify,
    }
    impl Provider for Gate {
        async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
            Err(TaskError::Permission)
        }
        async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.entered.notify_one();
            self.release.notified().await;
            self.completed.notify_one();
            Ok(7)
        }
    }
    for backend in [Backend::Tape, Backend::Native] {
        let p = Arc::new(Gate::default());
        let cancel = Cancellation::default();
        let (out, ()) = tokio::join!(
            route(
                backend,
                "print(1)\ntools.record_total(2,3)\nprint(2)",
                &[],
                &[],
                p.clone(),
                ScriptLimits::default(),
                cancel.clone()
            ),
            async {
                p.entered.notified().await;
                cancel.cancel();
            }
        );
        let mut fallback = 0;
        choose(&out, &mut fallback);
        let RouteOutcome::Executed(out) = out else {
            panic!("cancelled task routed")
        };
        assert_eq!(out.status, Err(TaskError::Cancelled));
        assert_eq!(out.stdout, "1\n");
        assert_eq!(fallback, 0);
        assert_eq!(p.calls.load(Ordering::Relaxed), 1);
        p.release.notify_one();
        p.completed.notified().await;
        assert_eq!(p.calls.load(Ordering::Relaxed), 1);
    }
}

#[cfg(feature = "tool-json")]
#[test]
fn json_preparation_limits_are_resource_errors_on_both_backends() {
    use minipy_native::{
        codegen::CodeUnit,
        interpreter::InterpreterUnit,
        tool::{JsonLeaf, ResultSchema, ToolDeclaration},
    };
    let tools = [ToolDeclaration {
        id: 101,
        name: "fetch".into(),
        arguments: vec![],
        result: ResultSchema::JsonTyped(vec![(vec!["message".into()], JsonLeaf::String)]),
    }];
    for expr in [
        format!("d['message'] == '{}'", "a".repeat(4097)),
        format!("'{}' in d['message']", "a".repeat(257)),
        format!("'{}'", "a".repeat(4097)),
    ] {
        let source = format!("d=tools.fetch()\nprint({expr})");
        let a = CodeUnit::compile_with_declarations(&source, &[], &tools)
            .err()
            .unwrap();
        let b = InterpreterUnit::prepare_with_declarations(&source, &[], &tools)
            .err()
            .unwrap();
        assert_eq!(a, b);
        assert_eq!(a.kind, AdmissionKind::Resource);
    }
}

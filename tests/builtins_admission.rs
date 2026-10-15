use minipy_native::{
    buffer::Quote,
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    ir::{AdmissionKind, Type},
    provider::{Delays, TestProvider},
    runtime::{self, Cancellation, ScriptLimits},
};
use std::{rc::Rc, sync::Arc};

async fn both(source: &str, rows: usize, limits: ScriptLimits) -> (Result<(), TaskError>, String) {
    let data = vec![
        Quote {
            unit_price_cents: 1,
            quantity: 2
        };
        rows
    ];
    let a = Arc::new(TestProvider::new(data.clone(), Delays::default()));
    let b = Arc::new(TestProvider::new(data, Delays::default()));
    let native = runtime::execute_script(
        Rc::new(CodeUnit::compile_script(source, &[]).unwrap()),
        a.clone(),
        &[],
        limits,
        Cancellation::default(),
        None,
    )
    .await;
    let tape = interpreter::execute_script(
        Rc::new(InterpreterUnit::prepare_script(source, &[]).unwrap()),
        b.clone(),
        &[],
        limits,
        Cancellation::default(),
        None,
    )
    .await;
    assert_eq!(native.status, tape.status);
    assert_eq!(native.stdout, tape.stdout);
    assert_eq!(
        (a.load_count(), a.record_count()),
        (b.load_count(), b.record_count())
    );
    (native.status, native.stdout)
}

#[tokio::test]
async fn scalar_builtins_evaluate_in_order_and_preserve_errors() {
    let (status, stdout) = both(
        "print(abs(-8))\nprint(min(8, -3))\nprint(max(-9, -2))\nprint(min(2, 4) + max(2, 4))",
        0,
        ScriptLimits::default(),
    )
    .await;
    assert_eq!(status, Ok(()));
    assert_eq!(stdout, "8\n-3\n-2\n6\n");
    for source in [
        "print(1)\nprint(abs(-9223372036854775808))\ntools.record_total(1,1)",
        "print(1)\nprint(min(9223372036854775807 + 1, 0))\ntools.record_total(1,1)",
        "print(1)\nprint(max(0, 9223372036854775807 + 1))\ntools.record_total(1,1)",
    ] {
        let (status, stdout) = both(source, 0, ScriptLimits::default()).await;
        assert_eq!(status, Err(TaskError::Overflow));
        assert_eq!(stdout, "1\n");
    }
    let (status, stdout) = both(
        "if False and abs(-9223372036854775808) > 0:\n    print(0)\nprint(7)",
        0,
        ScriptLimits::default(),
    )
    .await;
    assert_eq!(status, Ok(()));
    assert_eq!(stdout, "7\n");
}

#[tokio::test]
async fn buffer_len_survives_waits_without_scanning_or_spending_row_fuel() {
    for rows in [0, 3] {
        let source = "q=tools.load_quotes(1)\nprint(len(q))\nreceipt=tools.record_total(2, len(q))\nprint(max(len(q), receipt))";
        let mut limits = ScriptLimits::default();
        limits.limits.rows = 0;
        let (status, stdout) = both(source, rows, limits).await;
        assert_eq!(status, Ok(()));
        assert_eq!(stdout, format!("{rows}\n{}\n", rows + 2));
    }
    let source = "q=tools.load_quotes(1)\nfor r in q:\n    print(len(q) + r.quantity)";
    let (status, stdout) = both(source, 3, ScriptLimits::default()).await;
    assert_eq!(status, Ok(()));
    assert_eq!(stdout, "5\n5\n5\n");
    let source = "q=tools.load_quotes(1)\ntools.record_total(1,1)\nif len(q) > 0:\n    print(len(q))\nelse:\n    print(0)";
    for rows in [0, 3] {
        let (status, stdout) = both(source, rows, ScriptLimits::default()).await;
        assert_eq!(status, Ok(()));
        assert_eq!(stdout, format!("{rows}\n"));
    }
}

#[test]
fn preparation_rejects_before_effects_with_typed_locations() {
    for (source, kind, location, reason) in [
        (
            "print(1)\nprint(sum(x for x in []))\ntools.record_total(1,1)",
            AdmissionKind::Unsupported,
            (2, 7),
            "unsupported builtin 'sum'",
        ),
        (
            "print(1)\nprint(abs(True))\ntools.record_total(1,1)",
            AdmissionKind::Type,
            (2, 7),
            "abs requires i64",
        ),
        (
            "print(1)\nprint(len(4))\ntools.record_total(1,1)",
            AdmissionKind::Unsupported,
            (2, 7),
            "len requires",
        ),
        (
            "print(1)\nprint(min(1,))\ntools.record_total(1,1)",
            AdmissionKind::Unsupported,
            (2, 7),
            "min expects",
        ),
        (
            "print(1)\nprint(1 + )\ntools.record_total(1,1)",
            AdmissionKind::Syntax,
            (2, 11),
            "Python syntax error",
        ),
    ] {
        for error in [
            InterpreterUnit::prepare_script(source, &[]).err().unwrap(),
            CodeUnit::compile_script(source, &[]).err().unwrap(),
        ] {
            assert_eq!(error.kind, kind, "{source}: {error}");
            assert_eq!(error.location, Some(location), "{source}: {error}");
            assert!(error.reason.contains(reason), "{source}: {error}");
        }
    }
    let error = InterpreterUnit::prepare_script("print(abs(x))", &[("x", Type::Bool)])
        .err()
        .unwrap();
    assert_eq!(error.kind, AdmissionKind::Type);
    let error = InterpreterUnit::prepare_script("# 中文\nprint(sum(1, 2))", &[])
        .err()
        .unwrap();
    assert_eq!(error.location, Some((2, 7)));
    let error = InterpreterUnit::prepare_script("print(abs(2))", &[("abs", Type::I64)])
        .err()
        .unwrap();
    assert_eq!(error.kind, AdmissionKind::Schema);
    assert_eq!(error.location, None); // Host schema has no source position.
}

use minipy_native::{
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    provider::{Delays, TestProvider},
    runtime::{self, Cancellation, ScriptLimits},
};
use std::{rc::Rc, sync::Arc};

#[tokio::test]
async fn checked_literals_preserve_boundaries_bases_and_error_order() {
    let cases = [
        (
            "print(0)\nprint(-0)\nprint(7)\nprint(-7)",
            "0\n0\n7\n-7\n",
            Ok(()),
        ),
        (
            "print(9223372036854775807)\nprint(-9223372036854775808)",
            "9223372036854775807\n-9223372036854775808\n",
            Ok(()),
        ),
        (
            "print(0x7fff_ffff_ffff_ffff)\nprint(-0x8000_0000_0000_0000)\nprint(0o17)\nprint(0b101)",
            "9223372036854775807\n-9223372036854775808\n15\n5\n",
            Ok(()),
        ),
        (
            "print(9)\nprint(--9223372036854775808)\ntools.record_total(1,2)",
            "9\n",
            Err(TaskError::Overflow),
        ),
        (
            "print(False and --9223372036854775808 > 0)\nprint(True or 9223372036854775807 + 1 > 0)",
            "False\nTrue\n",
            Ok(()),
        ),
        (
            "print(9)\nprint(9223372036854775807 + 1)",
            "9\n",
            Err(TaskError::Overflow),
        ),
    ];
    for (source, stdout, status) in cases {
        let p = Arc::new(TestProvider::new(vec![], Delays::default()));
        let tape = Rc::new(InterpreterUnit::prepare_script(source, &[]).unwrap());
        let native = Rc::new(CodeUnit::compile_script(source, &[]).unwrap());
        let a = interpreter::execute_script(
            tape,
            p.clone(),
            &[],
            ScriptLimits::default(),
            Cancellation::default(),
            None,
        )
        .await;
        let b = runtime::execute_script(
            native,
            p.clone(),
            &[],
            ScriptLimits::default(),
            Cancellation::default(),
            None,
        )
        .await;
        for out in [a, b] {
            assert_eq!(out.status, status, "{source}");
            assert_eq!(out.stdout, stdout, "{source}");
        }
        assert_eq!(p.record_count(), 0);
    }
    for literal in [
        "9223372036854775808",
        "-9223372036854775809",
        "18446744073709551616",
        "999999999999999999999999999999999999999999999999999999999999999999999999999999",
        "-0x8000000000000001",
    ] {
        let source = format!("print(9)\nprint({literal})");
        let a = InterpreterUnit::prepare_script(&source, &[]).err().unwrap();
        let b = CodeUnit::compile_script(&source, &[]).err().unwrap();
        assert_eq!(a.reason, "integer literal is outside i64");
        assert_eq!(a.kind, minipy_native::ir::AdmissionKind::Unsupported);
        assert_eq!(
            a.location,
            Some((2, if literal.starts_with('-') { 8 } else { 7 }))
        );
        assert_eq!(b, a);
    }
}

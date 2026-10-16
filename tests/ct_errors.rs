#![cfg(feature = "tool-json")]
use minipy_native::{
    codegen::CodeUnit,
    interpreter::InterpreterUnit,
    ir::AdmissionKind,
    tool::{ArgumentSchema, JsonLeaf, ResultSchema, ToolDeclaration, ToolEnvironment},
};
#[test]
fn structured_errors_preserve_priority_and_source_location() {
    for typed in [false, true] {
        let paths = vec![vec!["nested".into(), "value".into()]];
        let tools = vec![
            ToolDeclaration {
                id: 2,
                name: "fetch".into(),
                arguments: vec![],
                result: if typed {
                    ResultSchema::JsonTyped(paths.into_iter().map(|p| (p, JsonLeaf::I64)).collect())
                } else {
                    ResultSchema::Json(paths)
                },
            },
            ToolDeclaration {
                id: 3,
                name: "number".into(),
                arguments: vec![ArgumentSchema::I64("n".into())],
                result: ResultSchema::I64,
            },
            ToolDeclaration {
                id: 4,
                name: "text".into(),
                arguments: vec![ArgumentSchema::String("s".into())],
                result: ResultSchema::I64,
            },
        ];
        let env = ToolEnvironment::new(tools.clone()).unwrap();
        // Exact structured diagnostics captured from the fixed CS baseline.
        for (source, kind, reason, location) in [
            (
                "r=tools.fetch()\nprint(r['absent'])",
                AdmissionKind::Unsupported,
                "JSON field path is absent from the declared schema",
                (2, 1),
            ),
            (
                "r=tools.fetch()\nprint(r['nested'])",
                AdmissionKind::Unsupported,
                "JSON expression must name a declared leaf",
                (2, 7),
            ),
            (
                "print(tools.number(True, missing=unknown))",
                AdmissionKind::Type,
                "Tool argument must be i64",
                (1, 1),
            ),
            (
                "print(tools.number(1, n=unknown))",
                AdmissionKind::Unsupported,
                "duplicate Tool argument",
                (1, 1),
            ),
            (
                "print(tools.number(missing=unknown))",
                AdmissionKind::Unsupported,
                "unknown Tool argument name",
                (1, 1),
            ),
            (
                "print(tools.text(n=unknown))",
                AdmissionKind::Unsupported,
                "string Tool requires its declared argument",
                (1, 1),
            ),
            (
                "r=tools.fetch()\nx=r['nested']['value']\nprint(tools.text(x))",
                AdmissionKind::Type,
                "string Tool requires a string literal or declared JSON leaf",
                (3, 1),
            ),
        ] {
            let a = InterpreterUnit::prepare_with_environment(source, &[], &env)
                .err()
                .unwrap();
            let b = CodeUnit::compile_with_environment(source, &[], &env)
                .err()
                .unwrap();
            let c = InterpreterUnit::prepare_with_declarations(source, &[], &tools)
                .err()
                .unwrap();
            assert_eq!(a, b);
            assert_eq!(a, c);
            assert_eq!(a.kind, kind);
            assert_eq!(a.reason, reason);
            assert_eq!(a.location, Some(location));
            println!(
                "CTERROR {}",
                serde_json::json!({"typed":typed,"source":source,"kind":format!("{:?}",a.kind),"reason":a.reason,"location":a.location})
            );
        }
    }
}

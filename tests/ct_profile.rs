//! Registration diagnostics are feature-gated and never part of formal CT timing.
#![cfg(all(
    feature = "tool-json",
    feature = "compiler-profile",
    not(target_arch = "wasm32")
))]
use minipy_native::{
    codegen::CodeUnit,
    compiler_profile::{self, Profile},
    interpreter::InterpreterUnit,
    tool::{ArgumentSchema, ResultSchema, ToolDeclaration, ToolEnvironment, ToolSchema},
};
fn declarations() -> Vec<ToolDeclaration> {
    vec![ToolDeclaration {
        id: 2,
        name: "number".into(),
        arguments: vec![ArgumentSchema::I64("n".into())],
        result: ResultSchema::I64,
    }]
}
fn nesting(p: &Profile) {
    assert!(p.parse_ns > 0);
    assert!(p.type_ns > 0);
    assert!(
        p.frontend_ns >= p.parse_ns + p.type_ns,
        "nested timers: {p:?}"
    );
}
#[test]
fn registered_and_declaration_frontend_timers_are_nested() {
    let tools = declarations();
    let env = ToolEnvironment::new(tools.clone()).unwrap();
    let schemas = vec![ToolSchema {
        id: 2,
        name: "number".into(),
        arguments: vec!["n".into()],
        result: ResultSchema::I64,
    }];
    for entry in 0..3 {
        compiler_profile::reset(false);
        let source = "value=tools.number(7)\nprint(value + 1)";
        let code = match entry {
            0 => CodeUnit::compile_with_environment(source, &[], &env),
            1 => CodeUnit::compile_with_declarations(source, &[], &tools),
            _ => CodeUnit::compile_with_tools(source, &[], &schemas),
        }
        .unwrap();
        let p = compiler_profile::take();
        nesting(&p);
        assert!(p.backend_ns > 0);
        assert!(p.finalize_ns > 0);
        println!(
            "CTPROFILE entry={entry} frontend={} parse={} type={} lowering={} define={} finalize={}",
            p.frontend_ns, p.parse_ns, p.type_ns, p.lowering_ns, p.backend_ns, p.finalize_ns
        );
        drop(code);
    }
    compiler_profile::reset(false);
    InterpreterUnit::prepare_with_environment("print(tools.number(7))", &[], &env).unwrap();
    let p = compiler_profile::take();
    assert!(p.parse_ns > 0);
    assert!(p.type_ns > 0);
    assert_eq!(p.backend_ns, 0);
    assert_eq!(p.frontend_ns, 0); // outer native timer is not a tape timer
}
#[test]
fn registration_error_diagnostics_stop_at_the_failed_phase() {
    let env = ToolEnvironment::new(declarations()).unwrap();
    for (source, parsed, typed) in [("print(", true, false), ("print(missing)", true, true)] {
        compiler_profile::reset(false);
        assert!(CodeUnit::compile_with_environment(source, &[], &env).is_err());
        let p = compiler_profile::take();
        assert_eq!(p.parse_ns > 0, parsed);
        assert_eq!(p.type_ns > 0, typed);
        assert!(p.frontend_ns >= p.parse_ns + p.type_ns);
        assert_eq!(p.backend_ns, 0);
        assert_eq!(p.lowering_ns, 0);
        assert_eq!(p.finalize_ns, 0);
    }
    let mut invalid = declarations();
    invalid[0].id = 0;
    compiler_profile::reset(false);
    assert!(CodeUnit::compile_with_declarations("print(1)", &[], &invalid).is_err());
    let p = compiler_profile::take();
    assert!(p.frontend_ns > 0);
    assert_eq!(p.parse_ns, 0);
    assert_eq!(p.type_ns, 0);
    assert_eq!(p.backend_ns, 0);
}

#![cfg(feature = "tool-json")]
use minipy_native::{
    buffer::QuoteBuffer,
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
    tool::{ArgumentSchema, Reply, ResultSchema, ToolArguments, ToolDeclaration, ToolEnvironment},
};
use std::{rc::Rc, sync::Arc};

fn declarations() -> Vec<ToolDeclaration> {
    vec![ToolDeclaration {
        id: 2,
        name: "metric".into(),
        arguments: vec![ArgumentSchema::I64("value".into())],
        result: ResultSchema::I64,
    }]
}
struct Host(bool);
impl Provider for Host {
    fn allows_tool(&self, _: u32) -> bool {
        self.0
    }
    async fn call_bound_tool(&self, id: u32, _: ToolArguments) -> Result<Reply, TaskError> {
        if id == 3 {
            Ok(Reply::Json(bytes::Bytes::from_static(
                b"{\"text\":\"hello\"}",
            )))
        } else {
            Ok(Reply::I64(42))
        }
    }
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        Err(TaskError::Permission)
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
}
#[test]
fn registered_environment_is_reusable_but_not_authority() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let mut tools = declarations();
    tools.push(ToolDeclaration {
        id: 3,
        name: "text".into(),
        arguments: vec![ArgumentSchema::String("path".into())],
        result: ResultSchema::JsonTyped(vec![(
            vec!["text".into()],
            minipy_native::tool::JsonLeaf::String,
        )]),
    });
    let env = ToolEnvironment::new(tools).unwrap();
    let source = "print(tools.metric(7))\ndata = tools.text('path')\nprint(data['text'])";
    let tape = Rc::new(InterpreterUnit::prepare_with_environment(source, &[], &env).unwrap());
    let native = Rc::new(CodeUnit::compile_with_environment(source, &[], &env).unwrap());
    assert!(InterpreterUnit::prepare_with_environment("print(tools.metric(8))", &[], &env).is_ok());
    drop(env);
    for allowed in [true, false] {
        let host = Arc::new(Host(allowed));
        let a = rt.block_on(interpreter::execute_script(
            tape.clone(),
            host.clone(),
            &[],
            ScriptLimits::default(),
            Cancellation::default(),
            None,
        ));
        let b = rt.block_on(runtime::execute_script(
            native.clone(),
            host,
            &[],
            ScriptLimits::default(),
            Cancellation::default(),
            None,
        ));
        assert_eq!(a.status, b.status);
        assert_eq!(a.stdout, b.stdout);
        if allowed {
            assert_eq!(a.status, Ok(()));
            assert_eq!(a.stdout, "42\nhello\n");
        } else {
            assert_eq!(a.status, Err(TaskError::Permission));
            assert!(a.stdout.is_empty());
        }
    }
}
#[test]
fn registration_and_source_errors_match_legacy_entry() {
    let mut invalid = declarations();
    invalid[0].id = 0;
    let old = ToolDeclaration::validate_all(&invalid)
        .unwrap_err()
        .to_string();
    assert_eq!(ToolEnvironment::new(invalid).unwrap_err().to_string(), old);
    let tools = declarations();
    let env = ToolEnvironment::new(tools.clone()).unwrap();
    for source in [
        "return 1",
        "print(tools.metric('x'))",
        "print(missing)",
        "print(tools.unknown(1))",
    ] {
        let a = InterpreterUnit::prepare_with_declarations(source, &[], &tools)
            .err()
            .unwrap()
            .to_string();
        let b = InterpreterUnit::prepare_with_environment(source, &[], &env)
            .err()
            .unwrap()
            .to_string();
        let c = CodeUnit::compile_with_environment(source, &[], &env)
            .err()
            .unwrap()
            .to_string();
        assert_eq!(a, b);
        assert_eq!(a, c);
    }
}

#[test]
#[ignore]
fn resident_new_source_cost() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let host = Arc::new(Host(true));
    let mut rows = Vec::new();
    for count in [1, 16] {
        let tools: Vec<_> = (0..count)
            .map(|i| ToolDeclaration {
                id: 2 + i as u32,
                name: format!("metric{i}"),
                arguments: vec![ArgumentSchema::I64("value".into())],
                result: ResultSchema::I64,
            })
            .collect();
        let setup = std::time::Instant::now();
        let environment = ToolEnvironment::new(tools.clone()).unwrap();
        let registration_ns = setup.elapsed().as_nanos();
        println!("REGISTER {{\"tools\":{count},\"ns\":{registration_ns}}}");
        for native in [false, true] {
            for used in [false, true] {
                for round in 0..4 {
                    for sample in 0..41 {
                        let source = if used {
                            format!("print(tools.metric0({sample}))")
                        } else {
                            format!("print({sample})")
                        };
                        let order = if (sample + round) % 2 == 0 {
                            [false, true]
                        } else {
                            [true, false]
                        };
                        for registered in order {
                            let start = std::time::Instant::now();
                            let outcome = if native {
                                let code = if registered {
                                    CodeUnit::compile_with_environment(&source, &[], &environment)
                                } else {
                                    CodeUnit::compile_with_declarations(&source, &[], &tools)
                                }
                                .unwrap();
                                rt.block_on(runtime::execute_script(
                                    Rc::new(code),
                                    host.clone(),
                                    &[],
                                    ScriptLimits::default(),
                                    Cancellation::default(),
                                    None,
                                ))
                            } else {
                                let code = if registered {
                                    InterpreterUnit::prepare_with_environment(
                                        &source,
                                        &[],
                                        &environment,
                                    )
                                } else {
                                    InterpreterUnit::prepare_with_declarations(&source, &[], &tools)
                                }
                                .unwrap();
                                rt.block_on(interpreter::execute_script(
                                    Rc::new(code),
                                    host.clone(),
                                    &[],
                                    ScriptLimits::default(),
                                    Cancellation::default(),
                                    None,
                                ))
                            };
                            let elapsed = start.elapsed().as_nanos();
                            assert_eq!(outcome.status, Ok(()));
                            assert_eq!(
                                outcome.stdout,
                                format!("{}\n", if used { 42 } else { sample })
                            );
                            rows.push((count, native, used, round, sample, registered, elapsed));
                        }
                    }
                }
            }
        }
    }
    for (tools, native, used, round, sample, registered, ns) in rows {
        println!(
            "ENV {{\"tools\":{tools},\"native\":{native},\"used\":{used},\"round\":{round},\"sample\":{sample},\"registered\":{registered},\"ns\":{ns}}}"
        );
    }
}

//! Bounded resident-service source-to-result probe; explicitly invoked only.
#![cfg(feature = "tool-json")]
use minipy_native::{
    buffer::QuoteBuffer,
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
    tool::{ArgumentSchema, JsonLeaf, Reply, ResultSchema, ToolArguments, ToolDeclaration},
};
use std::{rc::Rc, sync::Arc, time::Instant};
struct Host(bytes::Bytes);
impl Provider for Host {
    fn allows_tool(&self, _: u32) -> bool {
        true
    }
    async fn call_bound_tool(&self, _: u32, _: ToolArguments) -> Result<Reply, TaskError> {
        Ok(Reply::Json(self.0.clone()))
    }
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        Err(TaskError::Permission)
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
}
#[test]
#[ignore]
fn resident_source_cost() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let body = "a".repeat(16384);
    let host = Arc::new(Host(bytes::Bytes::from(format!(
        "{{\"content\":\"{body}\"}}"
    ))));
    let declarations = [ToolDeclaration {
        id: 2,
        name: "fetch".into(),
        arguments: vec![ArgumentSchema::String("path".into())],
        result: ResultSchema::JsonTyped(vec![(vec!["content".into()], JsonLeaf::String)]),
    }];
    for native in [false, true] {
        for case in ["scalar", "declared", "format", "json"] {
            for sample in 0..101 {
                let value = sample + 1;
                let (source, expected) = match case {
                    "scalar" | "declared" => (format!("n={value}\nprint(n)"), format!("{value}\n")),
                    "format" => (
                        format!(
                            "n={value}\nprint(f\"{{n}}/{{n}}/{{n}}/{{n}}/{{n}}/{{n}}/{{n}}/{{n}}\")"
                        ),
                        vec![value.to_string(); 8].join("/") + "\n",
                    ),
                    _ => (
                        format!(
                            "n={value}\ndata=tools.fetch('key')\nprint('[' + data['content'] + ']')"
                        ),
                        format!("[{body}]\n"),
                    ),
                };
                let start = Instant::now();
                let outcome = if native {
                    let code = if case == "scalar" {
                        CodeUnit::compile_script(&source, &[])
                    } else {
                        CodeUnit::compile_with_declarations(&source, &[], &declarations)
                    }
                    .unwrap();
                    rt.block_on(runtime::execute_script(
                        Rc::new(code),
                        host.clone(),
                        &[],
                        ScriptLimits {
                            stdout_bytes: 65536,
                            ..ScriptLimits::default()
                        },
                        Cancellation::default(),
                        None,
                    ))
                } else {
                    let code = if case == "scalar" {
                        InterpreterUnit::prepare_script(&source, &[])
                    } else {
                        InterpreterUnit::prepare_with_declarations(&source, &[], &declarations)
                    }
                    .unwrap();
                    rt.block_on(interpreter::execute_script(
                        Rc::new(code),
                        host.clone(),
                        &[],
                        ScriptLimits {
                            stdout_bytes: 65536,
                            ..ScriptLimits::default()
                        },
                        Cancellation::default(),
                        None,
                    ))
                };
                let elapsed = start.elapsed().as_nanos();
                assert_eq!(outcome.status, Ok(()));
                assert_eq!(outcome.stdout, expected);
                println!(
                    "COST {{\"backend\":\"{}\",\"case\":\"{}\",\"sample\":{},\"ns\":{}}}",
                    if native { "native" } else { "tape" },
                    case,
                    sample,
                    elapsed
                );
            }
        }
    }
}

//! Small CX lifecycle comparison, not a broad benchmark or a before/after claim.
#[cfg(feature = "tool-json")]
fn main() {
    probe::run();
}
#[cfg(not(feature = "tool-json"))]
fn main() {
    eprintln!("requires --features tool-json");
}
#[cfg(feature = "tool-json")]
mod probe {
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
        fn allows_tool(&self, id: u32) -> bool {
            id == 101
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
    pub fn run() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let host = Arc::new(Host(bytes::Bytes::from_static(
            br#"{"message":"incident"}"#,
        )));
        let declarations = [ToolDeclaration {
            id: 101,
            name: "fetch".into(),
            arguments: vec![ArgumentSchema::I64("key".into())],
            result: ResultSchema::JsonTyped(vec![(vec!["message".into()], JsonLeaf::String)]),
        }];
        println!("backend,case,sample,ns");
        for sample in 0..33 {
            let mut cases = [
                (false, "eq"),
                (false, "contains"),
                (false, "scalar"),
                (true, "eq"),
                (true, "contains"),
                (true, "scalar"),
            ];
            if sample % 2 == 1 {
                cases.reverse();
            }
            for (native, case) in cases {
                let expr = match case {
                    "eq" => "d['message'] == 'incident'",
                    "contains" => "'incident' in d['message']",
                    _ => "n == n",
                };
                let source = format!("n={sample}\nd=tools.fetch(7)\nprint({expr})");
                let start = Instant::now();
                let out = if native {
                    let code =
                        CodeUnit::compile_with_declarations(&source, &[], &declarations).unwrap();
                    rt.block_on(runtime::execute_script(
                        Rc::new(code),
                        host.clone(),
                        &[],
                        ScriptLimits::default(),
                        Cancellation::default(),
                        None,
                    ))
                } else {
                    let code =
                        InterpreterUnit::prepare_with_declarations(&source, &[], &declarations)
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
                assert_eq!(out.status, Ok(()));
                assert_eq!(out.stdout, "True\n");
                drop(out);
                let elapsed = start.elapsed().as_nanos();
                println!(
                    "{},{case},{sample},{elapsed}",
                    if native { "native" } else { "tape" }
                );
            }
        }
    }
}

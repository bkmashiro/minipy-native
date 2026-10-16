//! DB bounded lifecycle diagnostic: direct prefix vs existing leaf alias.
#[cfg(not(feature = "tool-json"))]
fn main() {
    eprintln!("requires --features tool-json");
}
#[cfg(feature = "tool-json")]
fn main() {
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
    struct Host;
    impl Provider for Host {
        fn allows_tool(&self, id: u32) -> bool {
            id == 101
        }
        async fn call_bound_tool(&self, _: u32, _: ToolArguments) -> Result<Reply, TaskError> {
            Ok(Reply::Json(bytes::Bytes::from_static(
                br#"{"filename":"draft-note.txt"}"#,
            )))
        }
        async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
            Err(TaskError::Permission)
        }
        async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
            Err(TaskError::Permission)
        }
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let host = Arc::new(Host);
    let declarations = [ToolDeclaration {
        id: 101,
        name: "fetch".into(),
        arguments: vec![ArgumentSchema::I64("key".into())],
        result: ResultSchema::JsonTyped(vec![(vec!["filename".into()], JsonLeaf::String)]),
    }];
    println!("backend,case,sample,ns");
    for sample in 0..33 {
        let mut cases = [
            (false, "direct"),
            (false, "alias"),
            (true, "direct"),
            (true, "alias"),
        ];
        if sample % 2 == 1 {
            cases.reverse();
        }
        for (native, case) in cases {
            let source = match case {
                "direct" => {
                    format!("d=tools.fetch({sample})\nprint(d['filename'].startswith('draft-'))")
                }
                _ => format!(
                    "d=tools.fetch({sample})\ns=d['filename']\nprint(s.startswith('draft-'))"
                ),
            };
            let start = Instant::now();
            let out = if native {
                let unit =
                    CodeUnit::compile_with_declarations(&source, &[], &declarations).unwrap();
                rt.block_on(runtime::execute_script(
                    Rc::new(unit),
                    host.clone(),
                    &[],
                    ScriptLimits::default(),
                    Cancellation::default(),
                    None,
                ))
            } else {
                let unit = InterpreterUnit::prepare_with_declarations(&source, &[], &declarations)
                    .unwrap();
                rt.block_on(interpreter::execute_script(
                    Rc::new(unit),
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
            let ns = start.elapsed().as_nanos();
            println!(
                "{},{case},{sample},{ns}",
                if native { "native" } else { "tape" }
            );
        }
    }
}

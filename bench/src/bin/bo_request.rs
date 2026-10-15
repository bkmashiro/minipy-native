//! Resident worker: new-source preparation + full request, setup separately.
//! Usage: bo-request <tape|native> <scalar|quote|string|plain|joined> <reps>.
use bytes::Bytes;
use minipy_native::{
    buffer::{Quote, QuoteBuffer}, codegen::CodeUnit, error::TaskError,
    interpreter::{self, InterpreterUnit}, ir::Type, provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
    tool::{ArgumentSchema, JsonLeaf, Reply, ResultSchema, ToolArguments, ToolDeclaration, ToolSchema},
};
use std::{rc::Rc, sync::Arc, time::Instant};

const INPUTS: &[(&str, Type)] = &[("batch_id", Type::I64)];
const SCALAR: &str = "x = batch_id + 1\nprint(x)";
const QUOTE: &str = "rows = tools.load_quotes(batch_id)\nprint(len(rows))";
const STRING: &str = "data = tools.fetch(batch_id)\ntext = data[\"message\"]\nprint(text)";
const PLAIN: &str = "data = tools.read_file(\"notes.txt\")\nprint(data[\"message\"])";
const JOINED: &str = "data = tools.read_file(\"notes\" + \".txt\")\nprint(f\"{data['message']}\")";
fn schema() -> [ToolSchema; 1] {
    [ToolSchema {
        id: 101, name: "fetch".into(), arguments: vec!["batch_id".into()],
        result: ResultSchema::JsonTyped(vec![(vec!["message".into()], JsonLeaf::String)]),
    }]
}
fn string_schema() -> [ToolDeclaration; 1] {
    [ToolDeclaration {
        id: 101, name: "read_file".into(), arguments: vec![ArgumentSchema::String("path".into())],
        result: ResultSchema::JsonTyped(vec![(vec!["message".into()], JsonLeaf::String)]),
    }]
}
struct Ready {
    bytes: Bytes,
    quotes: QuoteBuffer,
}
impl Provider for Ready {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        Ok(self.quotes.clone())
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
    fn allows_tool(&self, id: u32) -> bool { id == 101 }
    async fn call_tool(&self, _: u32, _: [i64; 2], _: usize) -> Result<Reply, TaskError> {
        Ok(Reply::Json(self.bytes.clone()))
    }
    async fn call_bound_tool(&self, id: u32, args: ToolArguments) -> Result<Reply, TaskError> {
        match args {
            ToolArguments::Numeric(values, count) => self.call_tool(id, values, count).await,
            ToolArguments::String(s) if id == 101 && s.as_str() == "notes.txt" =>
                Ok(Reply::Json(self.bytes.clone())),
            _ => Err(TaskError::Permission),
        }
    }
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 4);
    let reps: usize = args[3].parse().unwrap();
    let setup = Instant::now();
    let provider = Arc::new(Ready {
        bytes: Bytes::from_static(br#"{"message":"h\u00e9llo from tool"}"#),
        quotes: QuoteBuffer::from_quotes(vec![Quote { unit_price_cents: 3, quantity: 2 }; 256]).unwrap(),
    });
    println!("{{\"kind\":\"setup\",\"ns\":{}}}", setup.elapsed().as_nanos());
    let (source, expected) = match args[2].as_str() {
        "scalar" => (SCALAR, "8\n"),
        "quote" => (QUOTE, "256\n"),
        "string" => (STRING, "héllo from tool\n"),
        "plain" => (PLAIN, "héllo from tool\n"),
        "joined" => (JOINED, "héllo from tool\n"),
        _ => panic!("unknown case"),
    };
    for _ in 0..reps + 1 {
        let start = Instant::now();
        let stdout = match args[1].as_str() {
            "native" => {
                let code = Rc::new(match args[2].as_str() {
                    "string" => CodeUnit::compile_with_tools(source, INPUTS, &schema()).unwrap(),
                    "plain" | "joined" => CodeUnit::compile_with_declarations(source, INPUTS, &string_schema()).unwrap(),
                    _ => CodeUnit::compile_script(source, INPUTS).unwrap(),
                });
                let result = runtime::execute_script(code, provider.clone(), &[7],
                    ScriptLimits::default(), Cancellation::default(), None).await;
                result.status.unwrap(); result.stdout
            }
            "tape" => {
                let code = Rc::new(match args[2].as_str() {
                    "string" => InterpreterUnit::prepare_with_tools(source, INPUTS, &schema()).unwrap(),
                    "plain" | "joined" => InterpreterUnit::prepare_with_declarations(source, INPUTS, &string_schema()).unwrap(),
                    _ => InterpreterUnit::prepare_script(source, INPUTS).unwrap(),
                });
                let result = interpreter::execute_script(code, provider.clone(), &[7],
                    ScriptLimits::default(), Cancellation::default(), None).await;
                result.status.unwrap(); result.stdout
            }
            _ => panic!("unknown backend"),
        };
        let ns = start.elapsed().as_nanos();
        assert_eq!(stdout, expected);
        println!("{{\"kind\":\"request\",\"ns\":{ns}}}");
    }
}

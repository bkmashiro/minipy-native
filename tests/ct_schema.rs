//! Bounded CT schema-borrowing probe. Formal timing has no allocator/profile hooks.
#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
use minipy_native::{
    buffer::QuoteBuffer,
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
    tool::{
        ArgumentSchema, JsonLeaf, Reply, ResultSchema, ToolArguments, ToolDeclaration,
        ToolEnvironment,
    },
};
use std::{rc::Rc, sync::Arc, time::Instant};

struct Host;
impl Provider for Host {
    fn allows_tool(&self, _: u32) -> bool {
        true
    }
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        Err(TaskError::Permission)
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
    async fn call_bound_tool(&self, id: u32, args: ToolArguments) -> Result<Reply, TaskError> {
        if id == 2 {
            // Producer work is inside the timed request, not retained across requests.
            Ok(Reply::Json(bytes::Bytes::from(format!(
                "{{\"v0\":7,\"v1\":9}}"
            ))))
        } else {
            if id == 4 {
                let ToolArguments::String(s) = args else {
                    panic!("string argument")
                };
                assert_eq!(s.as_str(), "ok");
            }
            Ok(Reply::I64(42))
        }
    }
}
fn environment(width: usize, typed: bool) -> ToolEnvironment {
    let paths: Vec<_> = (0..width).map(|n| vec![format!("v{n}")]).collect();
    ToolEnvironment::new(vec![
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
    ])
    .unwrap()
}
fn source(case: &str, sample: usize) -> (String, String, usize) {
    let mut source = format!("# fresh source {sample}\n");
    let mut expected = String::new();
    let width;
    if case.starts_with('w') {
        let parts: Vec<usize> = case[1..].split('_').map(|s| s.parse().unwrap()).collect();
        width = parts[0];
        for call in 0..parts[1] {
            source += &format!("r{call}=tools.fetch()\nprint(r{call}['v0'])\n");
            expected += "7\n";
            if parts[2] == 2 {
                source += &format!("print(r{call}['v1'])\n");
                expected += "9\n";
            }
        }
    } else {
        width = 2;
        let (body, output) = match case {
            "scalar" => ("print(1 + 2)", "3\n"),
            "numeric" => ("print(tools.number(7))", "42\n"),
            "string" => ("print(tools.text('ok'))", "42\n"),
            "path_error" => ("r=tools.fetch()\nprint(r['absent'])", ""),
            _ => panic!("unknown case"),
        };
        source += body;
        expected += output;
    }
    (source, expected, width)
}

#[test]
fn prepared_plans_outlive_both_schema_variants() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    for typed in [false, true] {
        let env = environment(64, typed);
        let (s, expected, _) = source("w64_4_2", 0);
        let tape = Rc::new(InterpreterUnit::prepare_with_environment(&s, &[], &env).unwrap());
        let native = Rc::new(CodeUnit::compile_with_environment(&s, &[], &env).unwrap());
        drop(env);
        let a = rt.block_on(interpreter::execute_script(
            tape,
            Arc::new(Host),
            &[],
            ScriptLimits::default(),
            Cancellation::default(),
            None,
        ));
        let b = rt.block_on(runtime::execute_script(
            native,
            Arc::new(Host),
            &[],
            ScriptLimits::default(),
            Cancellation::default(),
            None,
        ));
        assert_eq!(a.status, Ok(()));
        assert_eq!(b.status, a.status);
        assert_eq!(a.stdout, expected);
        assert_eq!(b.stdout, expected);
    }
}

#[test]
#[ignore = "explicit bounded CT timing under perf lock"]
fn request() {
    let case = std::env::var("CT_CASE").unwrap();
    let native = std::env::var("CT_BACKEND").unwrap() == "native";
    let (_, _, width) = source(&case, 0);
    let env = environment(width, true);
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let host = Arc::new(Host);
    let mut rows = Vec::new();
    for sample in 0..112 {
        let (s, expected, _) = source(&case, sample);
        let start = Instant::now();
        let result = if native {
            CodeUnit::compile_with_environment(&s, &[], &env).map(|code| {
                rt.block_on(runtime::execute_script(
                    Rc::new(code),
                    host.clone(),
                    &[],
                    ScriptLimits::default(),
                    Cancellation::default(),
                    None,
                ))
            })
        } else {
            InterpreterUnit::prepare_with_environment(&s, &[], &env).map(|code| {
                rt.block_on(interpreter::execute_script(
                    Rc::new(code),
                    host.clone(),
                    &[],
                    ScriptLimits::default(),
                    Cancellation::default(),
                    None,
                ))
            })
        };
        // Check the result before dropping it; these fixed assertions are timed in both arms.
        match &result {
            Ok(out) => {
                assert_ne!(case, "path_error");
                assert_eq!(out.status, Ok(()));
                assert_eq!(out.stdout, expected);
            }
            Err(e) => {
                assert_eq!(case, "path_error");
                assert!(e.reason.contains("absent"));
            }
        }
        drop(result);
        let ns = start.elapsed().as_nanos();
        if sample >= 64 {
            rows.push(serde_json::json!({"case":case,"native":native,"sample":sample,"ns":ns}));
        }
    }
    for row in rows {
        println!("CT {row}");
    }
}

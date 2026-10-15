use minipy_native::{
    codegen::CodeUnit,
    interpreter::{self, InterpreterUnit},
    ir::Type,
    provider::{Delays, TestProvider},
    runtime::{Cancellation, ScriptLimits},
};
use std::{rc::Rc, sync::Arc, time::Instant};

#[test]
fn input_schema_names_and_errors() {
    let names: Vec<_> = (0..32).map(|i| format!("input_{i}")).collect();
    let valid: Vec<_> = names.iter().map(|s| (s.as_str(), Type::I64)).collect();
    for inputs in [&valid[..0], &valid[..2], &valid[..32]] {
        assert!(InterpreterUnit::prepare_script("print(1)", inputs).is_ok());
        assert!(CodeUnit::compile_script("print(1)", inputs).is_ok());
    }
    for n in [2, 32] {
        let mut duplicate = valid[..n].to_vec();
        duplicate[n - 1].0 = duplicate[0].0;
        let a = InterpreterUnit::prepare_script("print(1)", &duplicate)
            .err()
            .unwrap();
        let b = CodeUnit::compile_script("print(1)", &duplicate)
            .err()
            .unwrap();
        assert_eq!(a.to_string(), b.to_string());
        assert!(a.to_string().contains("unique identifiers"));
    }
    for name in ["", "bad-name", "2bad", "tools"] {
        assert!(InterpreterUnit::prepare_script("print(1)", &[(name, Type::I64)]).is_err());
    }
}

#[test]
#[ignore = "bounded resident new-source timing probe"]
fn resident_request_cost() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let host = Arc::new(TestProvider::new(vec![], Delays::default()));
    let names: Vec<_> = (0..32).map(|i| format!("input_{i}")).collect();
    let schema: Vec<_> = names.iter().map(|s| (s.as_str(), Type::I64)).collect();
    let mut rows = Vec::new();
    for count in [0, 2, 32] {
        let values = vec![7; count];
        for backend in ["tape", "native"] {
            for sample in 0..101 {
                let source = if count == 0 {
                    format!("print({sample})")
                } else {
                    format!("print(input_0 + {sample})")
                };
                let start = Instant::now();
                let outcome = if backend == "tape" {
                    let unit = Rc::new(
                        InterpreterUnit::prepare_script(&source, &schema[..count]).unwrap(),
                    );
                    rt.block_on(interpreter::execute_script(
                        unit,
                        host.clone(),
                        &values,
                        ScriptLimits::default(),
                        Cancellation::default(),
                        None,
                    ))
                } else {
                    let unit =
                        Rc::new(CodeUnit::compile_script(&source, &schema[..count]).unwrap());
                    rt.block_on(minipy_native::runtime::execute_script(
                        unit,
                        host.clone(),
                        &values,
                        ScriptLimits::default(),
                        Cancellation::default(),
                        None,
                    ))
                };
                let ns = start.elapsed().as_nanos();
                assert_eq!(outcome.status, Ok(()));
                assert_eq!(
                    outcome.stdout,
                    format!("{}\n", sample + if count == 0 { 0 } else { 7 })
                );
                rows.push(format!("INPUT {{\"inputs\":{count},\"backend\":\"{backend}\",\"sample\":{sample},\"ns\":{ns}}}"));
            }
        }
    }
    for row in rows {
        println!("{row}");
    }
}

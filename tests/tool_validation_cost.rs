#![cfg(feature = "tool-json")]

use minipy_native::tool::{ArgumentSchema, ResultSchema, ToolDeclaration, ToolSchema};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Allocator;
thread_local! {
    static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNT.with(|n| {
            if let Some(count) = n.get() {
                n.set(Some(count + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

fn allocations(f: impl FnOnce()) -> usize {
    COUNT.with(|n| n.set(Some(0)));
    f();
    COUNT.with(|n| n.replace(None).unwrap())
}

#[test]
fn declaration_validation_borrows_the_schema() {
    let numeric = vec![ToolSchema {
        id: 2,
        name: "fetch".into(),
        arguments: vec!["batch".into()],
        result: ResultSchema::Json(vec![vec!["nested".into(), "value".into()]]),
    }];
    let declarations: Vec<_> = numeric.iter().map(ToolDeclaration::from).collect();
    let baseline = allocations(|| ToolSchema::validate_all(&numeric).unwrap());
    let actual = allocations(|| ToolDeclaration::validate_all(&declarations).unwrap());
    println!("declaration allocations: numeric={baseline}, declaration={actual}");
    assert_eq!(
        actual, baseline,
        "validation must not clone declaration data"
    );
}

#[test]
fn scalar_sink_does_not_allocate_per_piece() {
    use minipy_native::{
        interpreter::{self, InterpreterUnit},
        provider::{Delays, TestProvider},
        runtime::{Cancellation, ScriptLimits},
    };
    use std::{rc::Rc, sync::Arc};
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let tools = [ToolDeclaration {
        id: 2,
        name: "unused".into(),
        arguments: vec![],
        result: ResultSchema::I64,
    }];
    let mut counts = Vec::new();
    for source in [
        "n=0\nprint(f\"{n}0000000\")",
        "n=0\nprint(f\"{n}{n}{n}{n}{n}{n}{n}{n}\")",
    ] {
        let unit =
            Rc::new(InterpreterUnit::prepare_with_declarations(source, &[], &tools).unwrap());
        let host = Arc::new(TestProvider::new(vec![], Delays::default()));
        let warm = rt.block_on(interpreter::execute_script(
            unit.clone(),
            host.clone(),
            &[],
            ScriptLimits::default(),
            Cancellation::default(),
            None,
        ));
        assert_eq!(warm.status, Ok(()));
        let count = allocations(|| {
            let outcome = rt.block_on(interpreter::execute_script(
                unit,
                host,
                &[],
                ScriptLimits::default(),
                Cancellation::default(),
                None,
            ));
            assert_eq!(outcome.status, Ok(()));
            assert_eq!(outcome.stdout, "00000000\n");
        });
        counts.push(count);
    }
    println!(
        "scalar sink allocations: one={}, eight={}",
        counts[0], counts[1]
    );
    assert_eq!(
        counts[0], counts[1],
        "scalar pieces must not each allocate a String"
    );
}

#[test]
fn unused_registered_tools_do_not_add_preparation_allocations() {
    use minipy_native::{interpreter::InterpreterUnit, tool::ToolEnvironment};
    let mut counts = Vec::new();
    for count in [1, 16] {
        let declarations = (0..count)
            .map(|i| ToolDeclaration {
                id: 2 + i,
                name: format!("metric{i}"),
                arguments: vec![ArgumentSchema::I64("value".into())],
                result: ResultSchema::I64,
            })
            .collect();
        let environment = ToolEnvironment::new(declarations).unwrap();
        // Initialize parser thread-local state before allocation accounting.
        InterpreterUnit::prepare_with_environment("print(1)", &[], &environment).unwrap();
        counts.push(allocations(|| {
            InterpreterUnit::prepare_with_environment("print(2)", &[], &environment).unwrap();
        }));
    }
    assert_eq!(counts[0], counts[1]);
}

#[test]
fn synchronous_join_does_not_promote_decoded_text_to_arc() {
    use minipy_native::{
        buffer::QuoteBuffer,
        codegen::CodeUnit,
        error::TaskError,
        interpreter::{self, InterpreterUnit},
        provider::Provider,
        runtime::{self, Cancellation, ScriptLimits},
        tool::{JsonLeaf, Reply, ToolArguments},
    };
    use std::{rc::Rc, sync::Arc};
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
        async fn call_bound_tool(&self, _: u32, _: ToolArguments) -> Result<Reply, TaskError> {
            Ok(Reply::Json(bytes::Bytes::from_static(
                br#"{"text":"hello\nworld"}"#,
            )))
        }
    }
    let tools = [ToolDeclaration {
        id: 2,
        name: "fetch".into(),
        arguments: vec![],
        result: ResultSchema::JsonTyped(vec![(vec!["text".into()], JsonLeaf::String)]),
    }];
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let mut borrowing = true;
    for native in [false, true] {
        let mut counts = Vec::new();
        for print in ["print(text)", "print(text + '')"] {
            let source = format!("data=tools.fetch()\ntext=data['text']\n{print}");
            let host = Arc::new(Host);
            if native {
                let code =
                    Rc::new(CodeUnit::compile_with_declarations(&source, &[], &tools).unwrap());
                assert_eq!(
                    rt.block_on(runtime::execute_script(
                        code.clone(),
                        host.clone(),
                        &[],
                        ScriptLimits::default(),
                        Cancellation::default(),
                        None
                    ))
                    .status,
                    Ok(())
                );
                counts.push(allocations(|| {
                    let out = rt.block_on(runtime::execute_script(
                        code,
                        host,
                        &[],
                        ScriptLimits::default(),
                        Cancellation::default(),
                        None,
                    ));
                    assert_eq!(out.status, Ok(()));
                    assert_eq!(out.stdout, "hello\nworld\n");
                }));
            } else {
                let code = Rc::new(
                    InterpreterUnit::prepare_with_declarations(&source, &[], &tools).unwrap(),
                );
                assert_eq!(
                    rt.block_on(interpreter::execute_script(
                        code.clone(),
                        host.clone(),
                        &[],
                        ScriptLimits::default(),
                        Cancellation::default(),
                        None
                    ))
                    .status,
                    Ok(())
                );
                counts.push(allocations(|| {
                    let out = rt.block_on(interpreter::execute_script(
                        code,
                        host,
                        &[],
                        ScriptLimits::default(),
                        Cancellation::default(),
                        None,
                    ));
                    assert_eq!(out.status, Ok(()));
                    assert_eq!(out.stdout, "hello\nworld\n");
                }));
            }
        }
        println!(
            "decoded print allocations native={native}: plain={}, joined={}",
            counts[0], counts[1]
        );
        borrowing &= counts[0] == counts[1];
    }
    assert!(borrowing, "synchronous join must borrow the decoded String");
}

#[test]
fn declaration_validation_preserves_errors_and_order() {
    let make = |name: &str, args| ToolDeclaration {
        id: 2,
        name: name.into(),
        arguments: args,
        result: ResultSchema::I64,
    };
    let mixed = vec![
        ArgumentSchema::String("path".into()),
        ArgumentSchema::I64("offset".into()),
    ];
    assert!(
        ToolDeclaration::validate_all(&[make("read", mixed.clone())])
            .unwrap_err()
            .to_string()
            .contains("one string argument")
    );
    assert!(
        ToolDeclaration::validate_all(&[make("load_quotes", mixed)])
            .unwrap_err()
            .to_string()
            .contains("unique and non-reserved")
    );
    assert!(
        ToolDeclaration::validate_all(&[make("read", vec![ArgumentSchema::String("path".into())])])
            .is_ok()
    );
}

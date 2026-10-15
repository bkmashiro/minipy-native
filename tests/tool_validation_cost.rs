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

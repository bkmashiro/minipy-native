//! Independent allocation diagnostic; never linked into the formal request binary.
#![cfg(feature = "tool-json")]
use minipy_native::{
    interpreter::InterpreterUnit,
    tool::{JsonLeaf, ResultSchema, ToolDeclaration, ToolEnvironment},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
struct Allocator;
thread_local! { static COUNT: Cell<Option<usize>> = const { Cell::new(None) }; }
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNT.with(|n| {
            if let Some(c) = n.get() {
                n.set(Some(c + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
fn counts() -> Vec<(bool, usize, usize, usize)> {
    let mut rows = Vec::new();
    for typed in [false, true] {
        for calls in [1, 4] {
            for width in [2, 64] {
                let paths: Vec<_> = (0..width).map(|n| vec![format!("v{n}")]).collect();
                let env = ToolEnvironment::new(vec![ToolDeclaration {
                    id: 2,
                    name: "fetch".into(),
                    arguments: vec![],
                    result: if typed {
                        ResultSchema::JsonTyped(
                            paths.into_iter().map(|p| (p, JsonLeaf::I64)).collect(),
                        )
                    } else {
                        ResultSchema::Json(paths)
                    },
                }])
                .unwrap();
                let source: String = (0..calls)
                    .map(|c| format!("r{c}=tools.fetch()\nprint(r{c}['v0'])\n"))
                    .collect();
                InterpreterUnit::prepare_with_environment(&source, &[], &env).unwrap();
                COUNT.with(|n| n.set(Some(0)));
                let result = InterpreterUnit::prepare_with_environment(&source, &[], &env);
                let count = COUNT.with(|n| n.replace(None).unwrap());
                drop(result.unwrap());
                rows.push((typed, calls, width, count));
            }
        }
    }
    rows
}
#[test]
fn matched_schema_width_does_not_add_allocations() {
    for pair in counts().chunks_exact(2) {
        assert_eq!(
            pair[0].3, pair[1].3,
            "unused leaves must stay borrowed: {pair:?}"
        );
    }
}
#[test]
#[ignore = "independent CT allocation diagnosis for both fixed source variants"]
fn allocation_diagnosis() {
    for (typed, calls, width, allocations) in counts() {
        println!(
            "CTALLOC {{\"typed\":{typed},\"calls\":{calls},\"width\":{width},\"allocations\":{allocations}}}"
        );
    }
}

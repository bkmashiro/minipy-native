//! Separate diagnostic overlay. Never enabled in formal builds.
use crate::{codegen::CodeUnit, interpreter::InterpreterUnit};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Allocator;
thread_local! {
    static COUNT: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}
fn account(bytes: usize) {
    COUNT.with(|n| {
        if let Some((calls, total)) = n.get() {
            n.set(Some((calls + 1, total + bytes)));
        }
    });
}
// Instrument the system allocator only in this single-threaded diagnostic.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        account(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        account(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, bytes: usize) -> *mut u8 {
        account(bytes);
        unsafe { System.realloc(ptr, layout, bytes) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

#[test]
#[ignore = "separate CJ prepare allocation census, not latency"]
fn cj_allocations() {
    let env = crate::interpreter::ci_probe::environment();
    for (name, source, inputs) in crate::interpreter::ci_probe::cases() {
        for native in [false, true] {
            for sample in 0..3 {
                // Per-case warmup initializes parser/compiler thread-local state.
                if native {
                    drop(CodeUnit::compile_with_environment(source, &inputs, &env).unwrap());
                } else {
                    drop(InterpreterUnit::prepare_with_environment(source, &inputs, &env).unwrap());
                }
                COUNT.with(|n| n.set(Some((0, 0))));
                let prepared = if native {
                    let unit = CodeUnit::compile_with_environment(source, &inputs, &env).unwrap();
                    drop(unit);
                    None
                } else {
                    let unit = InterpreterUnit::prepare_with_environment(source, &inputs, &env).unwrap();
                    let size = unit.prepared_bytes();
                    drop(unit);
                    Some(size)
                };
                let (allocations, requested_bytes) = COUNT.with(|n| n.replace(None).unwrap());
                println!("CJ_ALLOC {}", serde_json::json!({"case":name,"native":native,
                    "sample":sample,"allocations":allocations,"requested_bytes":requested_bytes,
                    "prepared_bytes":prepared}));
            }
        }
    }
}

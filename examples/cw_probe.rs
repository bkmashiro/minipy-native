//! Bounded CW fresh-source lifecycle probe; allocation diagnostics are a separate build.
use minipy_native::{
    codegen::CodeUnit,
    interpreter::{self, InterpreterUnit},
    provider::{Delays, TestProvider},
    runtime::{self, Cancellation, ScriptLimits},
};
use std::{rc::Rc, sync::Arc, time::Instant};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let native = args[1] == "native";
    let case = args[2].as_str();
    let count: usize = args[3].parse().unwrap();
    let (base, expected) = match case {
        "no_int" => ("print(True)\n".to_owned(), "True\n"),
        "short" => ("x = 17 * 3 + 2\nprint(x)\n".to_owned(), "53\n"),
        "dense" => (
            format!("{}print(x)\n", "x = -1234567890123456789\n".repeat(32)),
            "-1234567890123456789\n",
        ),
        _ => panic!("unknown case"),
    };
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let provider = Arc::new(TestProvider::new(vec![], Delays::default()));
    for i in 0..count + 8 {
        #[cfg(feature = "json-allocation-profile")]
        allocs::reset();
        let start = Instant::now();
        // Include fresh source creation, compilation, execution, assertions and teardown.
        let source = format!("{base}# request {i}\n");
        let out = if native {
            let code = Rc::new(CodeUnit::compile_script(&source, &[]).unwrap());
            rt.block_on(runtime::execute_script(
                code,
                provider.clone(),
                &[],
                ScriptLimits::default(),
                Cancellation::default(),
                None,
            ))
        } else {
            let code = Rc::new(InterpreterUnit::prepare_script(&source, &[]).unwrap());
            rt.block_on(interpreter::execute_script(
                code,
                provider.clone(),
                &[],
                ScriptLimits::default(),
                Cancellation::default(),
                None,
            ))
        };
        assert_eq!(out.status, Ok(()));
        assert_eq!(out.stdout, expected);
        drop(out);
        drop(source);
        let ns = start.elapsed().as_nanos();
        #[cfg(feature = "json-allocation-profile")]
        let (calls, bytes) = (allocs::calls(), allocs::bytes());
        #[cfg(not(feature = "json-allocation-profile"))]
        let (calls, bytes) = (0, 0);
        if i >= 8 {
            println!("{case},{native},{i},{ns},{calls},{bytes}");
        }
    }
}

#[cfg(feature = "json-allocation-profile")]
mod allocs {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
    };
    thread_local! {static CALLS: Cell<usize> = const {Cell::new(0)}; static BYTES: Cell<usize> = const {Cell::new(0)};}
    pub struct Counter;
    unsafe impl GlobalAlloc for Counter {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            CALLS.set(CALLS.get() + 1);
            BYTES.set(BYTES.get() + l.size());
            unsafe { System.alloc(l) }
        }
        unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
            CALLS.set(CALLS.get() + 1);
            BYTES.set(BYTES.get() + l.size());
            unsafe { System.alloc_zeroed(l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            CALLS.set(CALLS.get() + 1);
            BYTES.set(BYTES.get() + n);
            unsafe { System.realloc(p, l, n) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            unsafe { System.dealloc(p, l) }
        }
    }
    pub fn reset() {
        CALLS.set(0);
        BYTES.set(0);
    }
    pub fn calls() -> usize {
        CALLS.get()
    }
    pub fn bytes() -> usize {
        BYTES.get()
    }
}
#[cfg(feature = "json-allocation-profile")]
#[global_allocator]
static ALLOCATOR: allocs::Counter = allocs::Counter;

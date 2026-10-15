//! Bounded source-cold sink diagnostic; setup excluded, each request fresh.
use bytes::Bytes;
use minipy_native::{
    buffer::QuoteBuffer, codegen::CodeUnit, error::TaskError,
    interpreter::{self, InterpreterUnit}, provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
    tool::{ArgumentSchema, JsonLeaf, Reply, ResultSchema, ToolArguments, ToolDeclaration},
};
use std::{alloc::{GlobalAlloc, Layout, System}, rc::Rc, sync::{Arc, atomic::{AtomicUsize, Ordering}}, time::{Duration, Instant}};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(new_size, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;
struct Ready { reply: Bytes, gated: bool }
impl Provider for Ready {
    fn allows_tool(&self, id: u32) -> bool { id == 21 }
    async fn call_bound_tool(&self, id: u32, args: ToolArguments) -> Result<Reply, TaskError> {
        if id != 21 { return Err(TaskError::Permission); }
        let ToolArguments::String(s) = args else { return Err(TaskError::Permission) };
        assert!(s.as_str().ends_with(".txt"));
        if self.gated { tokio::time::sleep(Duration::from_millis(1)).await; }
        Ok(Reply::Json(self.reply.clone()))
    }
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> { Err(TaskError::Permission) }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> { Err(TaskError::Permission) }
}
fn schema() -> [ToolDeclaration; 1] {
    [ToolDeclaration {
        id: 21, name: "read_file".into(), arguments: vec![ArgumentSchema::String("path".into())],
        result: ResultSchema::JsonTyped(vec![(vec!["content".into()], JsonLeaf::String)]),
    }]
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 4, "bs-diag <tape|native> <short|long> <ready|gated>");
    let long = args[2] == "long";
    let gated = args[3] == "gated";
    let text = if long { "é".repeat(2048) } else { "héllo".to_owned() };
    let reply = Bytes::from(serde_json::to_vec(&serde_json::json!({"content": text})).unwrap());
    let host = Arc::new(Ready { reply, gated });
    for multi in [false, true] {
        let source = if multi {
            "data = tools.read_file(\"notes\" + \".txt\")\nprint(\"[\" + data[\"content\"] + \"]\")"
        } else {
            "data = tools.read_file(\"notes.txt\")\nprint(f\"[{data['content']}]\")"
        };
        let expected = format!("[{text}]\n");
        for n in 0..13 {
            ALLOCS.store(0, Ordering::Relaxed); BYTES.store(0, Ordering::Relaxed);
            let prep = Instant::now();
            if args[1] == "native" {
                let code = Rc::new(CodeUnit::compile_with_declarations(source, &[], &schema()).unwrap());
                let prep_ns = prep.elapsed().as_nanos();
                let prep_allocs = ALLOCS.load(Ordering::Relaxed);
                let prep_bytes = BYTES.load(Ordering::Relaxed);
                let exec = Instant::now();
                let out = runtime::execute_script(code, host.clone(), &[], ScriptLimits::default(), Cancellation::default(), None).await;
                let exec_ns = exec.elapsed().as_nanos();
                assert_eq!(out.status, Ok(())); assert_eq!(out.stdout, expected);
                println!("{{\"backend\":\"native\",\"length\":\"{}\",\"gate\":\"{}\",\"multi\":{},\"warmup\":{},\"prep_ns\":{prep_ns},\"exec_ns\":{exec_ns},\"prep_allocs\":{prep_allocs},\"prep_bytes\":{prep_bytes},\"exec_allocs\":{},\"exec_bytes\":{}}}", args[2], args[3], multi, n == 0, ALLOCS.load(Ordering::Relaxed)-prep_allocs, BYTES.load(Ordering::Relaxed)-prep_bytes);
            } else {
                let code = Rc::new(InterpreterUnit::prepare_with_declarations(source, &[], &schema()).unwrap());
                let prep_ns = prep.elapsed().as_nanos();
                let prep_allocs = ALLOCS.load(Ordering::Relaxed);
                let prep_bytes = BYTES.load(Ordering::Relaxed);
                let exec = Instant::now();
                let out = interpreter::execute_script(code, host.clone(), &[], ScriptLimits::default(), Cancellation::default(), None).await;
                let exec_ns = exec.elapsed().as_nanos();
                assert_eq!(out.status, Ok(())); assert_eq!(out.stdout, expected);
                println!("{{\"backend\":\"tape\",\"length\":\"{}\",\"gate\":\"{}\",\"multi\":{},\"warmup\":{},\"prep_ns\":{prep_ns},\"exec_ns\":{exec_ns},\"prep_allocs\":{prep_allocs},\"prep_bytes\":{prep_bytes},\"exec_allocs\":{},\"exec_bytes\":{}}}", args[2], args[3], multi, n == 0, ALLOCS.load(Ordering::Relaxed)-prep_allocs, BYTES.load(Ordering::Relaxed)-prep_bytes);
            }
        }
    }
}

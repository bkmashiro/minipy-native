//! Small serial allocation/future/request smoke. Not a throughput benchmark.
#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
use minipy_native::{
    buffer::QuoteBuffer,
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
    tool::{JsonLeaf, ObjectArraySchema, Reply, ResultSchema, ToolArguments, ToolDeclaration},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    rc::Rc,
    sync::Arc,
    time::Instant,
};
struct Allocator;
thread_local! { static COUNTS:Cell<Option<(usize,usize)>>=const {Cell::new(None)}; }
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        COUNTS.with(|c| {
            if let Some((n, b)) = c.get() {
                c.set(Some((n + 1, b + l.size())))
            }
        });
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
struct Host;
impl Provider for Host {
    fn allows_tool(&self, _: u32) -> bool {
        true
    }
    async fn call_bound_tool(&self, id: u32, _: ToolArguments) -> Result<Reply, TaskError> {
        Ok(Reply::Json(bytes::Bytes::from_static(if id == 201 {
            br#"{"content":"WARN:x\nno\nWARN:y"}"#
        } else {
            br#"{"items":[{"n":3},{"n":4}]}"#
        })))
    }
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        Err(TaskError::Permission)
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
}
#[tokio::test]
#[ignore = "bounded serial diagnostic under build lock"]
async fn request_costs() {
    let tools = vec![
        ToolDeclaration {
            id: 201,
            name: "log".into(),
            arguments: vec![],
            result: ResultSchema::JsonTyped(vec![(vec!["content".into()], JsonLeaf::String)]),
        },
        ToolDeclaration {
            id: 202,
            name: "items".into(),
            arguments: vec![],
            result: ResultSchema::JsonObjectArray(
                ObjectArraySchema {
                    path: vec!["items".into()],
                    i64_fields: vec!["n".into()],
                }
                .into(),
            ),
        },
    ];
    let sources = [
        ("scalar", "x=7\nprint(x+2)", "9\n"),
        (
            "json",
            "data=tools.log()\nprint(data.content)",
            "WARN:x\nno\nWARN:y\n",
        ),
        (
            "array",
            "data=tools.items()\nn=0\nfor row in data['items']:\n    n+=row['n']\nprint(n)",
            "7\n",
        ),
        (
            "lines_count",
            "data=tools.log()\nn=0\nfor line in data.content.splitlines():\n    if line.startswith('WARN:'):\n        n+=1\nprint(n)",
            "2\n",
        ),
        (
            "lines_print",
            "data=tools.log()\nfor line in data.content.splitlines():\n    if 'WARN:' in line:\n        print(line)",
            "WARN:x\nWARN:y\n",
        ),
    ];
    // Prime runtime/task infrastructure without including the measured sources.
    tokio::task::yield_now().await;
    for (name, source, expected) in sources {
        if std::env::var_os("DR_OLD_ONLY").is_some() && name.starts_with("lines_") {
            continue;
        }
        for native in [false, true] {
            let start = Instant::now();
            let host = Arc::new(Host);
            let cancel = Cancellation::default();
            let limits = ScriptLimits::default();
            let (out, size, prepared, code, allocations) = if native {
                let unit =
                    Rc::new(CodeUnit::compile_with_declarations(source, &[], &tools).unwrap());
                let code = unit.code_bytes;
                let future = runtime::execute_script(unit.clone(), host, &[], limits, cancel, None);
                let size = std::mem::size_of_val(&future);
                COUNTS.with(|c| c.set(Some((0, 0))));
                let out = future.await;
                let counts = COUNTS.with(|c| c.replace(None).unwrap());
                drop(unit);
                (out, size, 0, code, counts)
            } else {
                let unit = Rc::new(
                    InterpreterUnit::prepare_with_declarations(source, &[], &tools).unwrap(),
                );
                let prepared = unit.prepared_bytes();
                let future =
                    interpreter::execute_script(unit.clone(), host, &[], limits, cancel, None);
                let size = std::mem::size_of_val(&future);
                COUNTS.with(|c| c.set(Some((0, 0))));
                let out = future.await;
                let counts = COUNTS.with(|c| c.replace(None).unwrap());
                drop(unit);
                (out, size, prepared, 0, counts)
            };
            let ns = start.elapsed().as_nanos();
            assert_eq!(out.status, Ok(()));
            assert_eq!(out.stdout, expected);
            println!(
                "DRCOST {}",
                serde_json::json!({"source":name,"native":native,"future_bytes":size,"tape_prepared_bytes":prepared,"native_code_bytes":code,"execute_alloc_calls":allocations.0,"execute_alloc_requested_bytes":allocations.1,"fresh_prepare_execute_unit_drop_ns":ns})
            );
        }
    }
}

#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
use minipy_native::{
    buffer::{QuoteBuffer, RecordBuffer},
    codegen::CodeUnit,
    error::TaskError,
    frontend,
    interpreter::{self, InterpreterUnit},
    ir::Type,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
    tool::{RecordField, RecordSchema, Reply, ResultSchema, ToolSchema},
};
use std::rc::Rc;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::Notify;

fn schema(stride: usize, fields: &[(&str, Type, usize)]) -> RecordSchema {
    RecordSchema {
        stride,
        fields: fields
            .iter()
            .map(|(name, ty, offset)| RecordField {
                name: (*name).into(),
                ty: *ty,
                offset: *offset,
            })
            .collect(),
    }
}
fn declarations(s: RecordSchema) -> Vec<ToolSchema> {
    vec![
        ToolSchema {
            id: 43,
            name: "fetch_rows".into(),
            arguments: vec!["seed".into()],
            result: ResultSchema::Records(s),
        },
        ToolSchema {
            id: 44,
            name: "finish".into(),
            arguments: vec!["seed".into()],
            result: ResultSchema::I64,
        },
    ]
}
fn bytes(rows: &[&[i64]]) -> Vec<u8> {
    rows.iter()
        .flat_map(|row| row.iter().flat_map(|v| v.to_le_bytes()))
        .collect()
}
struct Source {
    schema: RecordSchema,
    data: Vec<u8>,
    calls: AtomicUsize,
    weak: Mutex<Option<Weak<Box<[i64]>>>>,
    denied: bool,
    mismatch: bool,
    released_before_finish: AtomicUsize,
    started: Option<Arc<Notify>>,
    resume: Option<Arc<Notify>>,
}
impl Source {
    fn new(schema: RecordSchema, data: Vec<u8>) -> Self {
        Self {
            schema,
            data,
            calls: AtomicUsize::new(0),
            weak: Mutex::new(None),
            denied: false,
            mismatch: false,
            released_before_finish: AtomicUsize::new(0),
            started: None,
            resume: None,
        }
    }
}
impl Provider for Source {
    fn allows_tool(&self, id: u32) -> bool {
        !self.denied && (id == 43 || id == 44)
    }
    async fn call_tool(&self, id: u32, args: [i64; 2], count: usize) -> Result<Reply, TaskError> {
        assert_eq!((args[0], count), (7, 1));
        self.calls.fetch_add(1, Ordering::Relaxed);
        if id == 44 {
            if self
                .weak
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|w| w.upgrade().is_none())
            {
                self.released_before_finish.store(1, Ordering::Relaxed);
            }
            return Ok(Reply::I64(1));
        }
        assert_eq!(id, 43);
        if let (Some(started), Some(resume)) = (&self.started, &self.resume) {
            started.notify_one();
            resume.notified().await;
        }
        let mut schema = self.schema.clone();
        if self.mismatch {
            schema.fields[0].name.push('x');
        }
        let buffer = RecordBuffer::import_bytes(schema, &self.data)?;
        *self.weak.lock().unwrap() = Some(buffer.downgrade());
        Ok(Reply::Records(buffer))
    }
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        panic!("not a Quote load")
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        panic!("not a Quote record")
    }
}
async fn run(
    native: bool,
    source: &str,
    host: Arc<Source>,
    options: ScriptLimits,
) -> runtime::ScriptOutcome {
    let decl = declarations(host.schema.clone());
    if native {
        let code =
            Rc::new(CodeUnit::compile_with_tools(source, &[("seed", Type::I64)], &decl).unwrap());
        runtime::execute_script(code, host, &[7], options, Cancellation::default(), None).await
    } else {
        let unit = Rc::new(
            InterpreterUnit::prepare_with_tools(source, &[("seed", Type::I64)], &decl).unwrap(),
        );
        interpreter::execute_script(unit, host, &[7], options, Cancellation::default(), None).await
    }
}

#[tokio::test]
async fn distinct_layouts_scan_on_both_backends_and_release_before_later_effect() {
    let cases = [
        (
            schema(24, &[("amount", Type::I64, 16), ("enabled", Type::Bool, 0)]),
            bytes(&[&[1, 50, 3], &[0, 99, -8], &[1, 0, 5]]),
            "rows = tools.fetch_rows(seed)\ntotal = 0\nfor item in rows:\n    if item.enabled:\n        total += item.amount\nprint(total)\nreceipt = tools.finish(seed)\nprint(receipt)",
            "8\n1\n",
        ),
        (
            schema(16, &[("delta", Type::I64, 0), ("weight", Type::I64, 8)]),
            bytes(&[&[4, 3], &[-2, 7]]),
            "rows = tools.fetch_rows(seed)\ntotal = 0\nfor item in rows:\n    total += item.delta * item.weight\nprint(total)\nreceipt = tools.finish(seed)\nprint(receipt)",
            "-2\n1\n",
        ),
    ];
    for native in [false, true] {
        for (schema, data, script, expected) in cases.clone() {
            let host = Arc::new(Source::new(schema, data));
            let result = run(
                native,
                script,
                host.clone(),
                ScriptLimits {
                    limits: runtime::Limits {
                        chunk: 1,
                        ..runtime::Limits::default()
                    },
                    ..ScriptLimits::default()
                },
            )
            .await;
            assert_eq!(result.status, Ok(()), "native={native}");
            assert_eq!(result.stdout, expected);
            assert_eq!(host.calls.load(Ordering::Relaxed), 2);
            assert_eq!(host.released_before_finish.load(Ordering::Relaxed), 1);
            assert!(
                host.weak
                    .lock()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .upgrade()
                    .is_none()
            );
        }
    }
}

#[tokio::test]
async fn authority_schema_budget_fuel_and_error_prefix() {
    let schema = schema(24, &[("ok", Type::Bool, 0), ("sum", Type::I64, 16)]);
    let data = bytes(&[&[1, 99, 2], &[1, 11, 3]]);
    let script = "print(1)\nrows = tools.fetch_rows(seed)\ntotal = 0\nfor row in rows:\n    total += row.sum\nprint(total)";
    for native in [false, true] {
        for (denied, mismatch, row_limit, byte_limit, expected) in [
            (true, false, 10, 100, TaskError::Permission),
            (false, true, 10, 100, TaskError::Buffer),
            (false, false, 10, 47, TaskError::Budget),
            (false, false, 1, 100, TaskError::Budget),
        ] {
            let mut source = Source::new(schema.clone(), data.clone());
            source.denied = denied;
            source.mismatch = mismatch;
            let host = Arc::new(source);
            let result = run(
                native,
                script,
                host.clone(),
                ScriptLimits {
                    limits: runtime::Limits {
                        rows: row_limit,
                        buffer_bytes: byte_limit,
                        chunk: 1,
                        ..runtime::Limits::default()
                    },
                    ..ScriptLimits::default()
                },
            )
            .await;
            assert_eq!(result.status, Err(expected), "native={native}");
            assert_eq!(result.stdout, "1\n");
            assert_eq!(host.calls.load(Ordering::Relaxed), usize::from(!denied));
            if !denied {
                assert!(
                    host.weak
                        .lock()
                        .unwrap()
                        .as_ref()
                        .unwrap()
                        .upgrade()
                        .is_none()
                );
            }
        }
    }
    // Two live record owners share the task budget before either scan starts.
    for native in [false, true] {
        let host = Arc::new(Source::new(schema.clone(), data.clone()));
        let result = run(
            native,
            "a = tools.fetch_rows(seed)\nb = tools.fetch_rows(seed)\nfor row in a:\n    print(row.sum)\nfor row in b:\n    print(row.sum)",
            host.clone(),
            ScriptLimits {
                limits: runtime::Limits { buffer_bytes: 48, ..runtime::Limits::default() },
                ..ScriptLimits::default()
            },
        ).await;
        assert_eq!(result.status, Err(TaskError::Budget), "native={native}");
        assert_eq!(host.calls.load(Ordering::Relaxed), 2);
    }
}

#[test]
fn record_admission_and_import_reject_invalid_layouts_and_bool_cells() {
    let s = schema(24, &[("ok", Type::Bool, 0), ("value", Type::I64, 16)]);
    for invalid in [
        schema(7, &[("x", Type::I64, 0)]),
        schema(16, &[("x", Type::I64, 16)]),
        schema(16, &[("x", Type::I64, 0), ("y", Type::I64, 0)]),
        schema(16, &[("x", Type::I64, 0), ("x", Type::Bool, 8)]),
        schema(
            16,
            &[("x", Type::I64, 0), (&"a".repeat(257), Type::Bool, 8)],
        ),
    ] {
        assert!(invalid.validate().is_err());
    }
    assert_eq!(
        RecordBuffer::import_bytes(s.clone(), &[0; 23]).unwrap_err(),
        TaskError::Buffer
    );
    assert_eq!(
        RecordBuffer::import_bytes(s.clone(), &bytes(&[&[2, 0, 4]])).unwrap_err(),
        TaskError::Buffer
    );
    assert_eq!(
        RecordBuffer::import_bytes(
            s.clone(),
            &vec![0; minipy_native::buffer::MAX_RECORD_BYTES + 8]
        )
        .unwrap_err(),
        TaskError::Budget
    );
    for source in [
        "rows = tools.fetch_rows(seed)\nfor r in rows:\n    print(r.missing)",
        "rows = tools.fetch_rows(seed)\nfor r in rows:\n    print(r.value + r.ok)",
        "rows = tools.fetch_rows(seed)\nfor r in rows:\n    print(r.unit_price_cents)",
        "print(tools.fetch_rows(seed))",
        "tools.fetch_rows(seed)",
    ] {
        assert!(
            frontend::parse_with_tools(source, &[("seed", Type::I64)], &declarations(s.clone()))
                .is_err(),
            "{source}"
        );
    }
}

#[tokio::test]
async fn cancellation_during_record_tool_drops_detached_reply() {
    let s = schema(16, &[("a", Type::I64, 0)]);
    for native in [false, true] {
        let started = Arc::new(Notify::new());
        let resume = Arc::new(Notify::new());
        let mut source = Source::new(s.clone(), bytes(&[&[3, 0]]));
        source.started = Some(started.clone());
        source.resume = Some(resume.clone());
        let host = Arc::new(source);
        let cancel = Cancellation::default();
        let decl = declarations(s.clone());
        let script = "print(1)\nrows = tools.fetch_rows(seed)\nfor row in rows:\n    print(row.a)";
        let effect = async {
            if native {
                let code = Rc::new(
                    CodeUnit::compile_with_tools(script, &[("seed", Type::I64)], &decl).unwrap(),
                );
                runtime::execute_script(
                    code,
                    host.clone(),
                    &[7],
                    ScriptLimits::default(),
                    cancel.clone(),
                    None,
                )
                .await
            } else {
                let code = Rc::new(
                    InterpreterUnit::prepare_with_tools(script, &[("seed", Type::I64)], &decl)
                        .unwrap(),
                );
                interpreter::execute_script(
                    code,
                    host.clone(),
                    &[7],
                    ScriptLimits::default(),
                    cancel.clone(),
                    None,
                )
                .await
            }
        };
        let interrupt = async {
            started.notified().await;
            cancel.cancel();
            resume.notify_one();
        };
        let (result, ()) = tokio::join!(effect, interrupt);
        assert_eq!(result.status, Err(TaskError::Cancelled));
        assert_eq!(result.stdout, "1\n");
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        assert_eq!(host.calls.load(Ordering::Relaxed), 1);
        assert!(
            host.weak
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|w| w.upgrade().is_none())
        );
    }
}

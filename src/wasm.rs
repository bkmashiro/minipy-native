//! Small single-worker JSON ABI. All i64 values cross JS as decimal strings.
use crate::{buffer::Quote, interpreter::debug::Engine, ir::Type, runtime::ScriptLimits};
use serde::Deserialize;
use serde_json::{Value, json};
use std::cell::RefCell;
thread_local! {
    static ENGINE: RefCell<Engine> = RefCell::new(Engine::default());
    static INPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Command {
    Prepare {
        source: String,
        #[serde(default)]
        inputs: Vec<(String, String)>,
    },
    Create {
        unit: usize,
        #[serde(default)]
        inputs: Vec<String>,
        #[serde(default)]
        limits: Options,
    },
    Advance {
        task: usize,
        count: usize,
        trace: bool,
    },
    Snapshot {
        task: usize,
    },
    Cancel {
        task: usize,
    },
    Complete {
        task: usize,
        request: u64,
        #[serde(default)]
        rows: Option<usize>,
        #[serde(default)]
        price: Option<String>,
        #[serde(default)]
        quantity: Option<String>,
        #[serde(default)]
        receipt: Option<String>,
        #[serde(default)]
        error: Option<String>,
    },
    Reset,
}
#[derive(Default, Deserialize)]
struct Options {
    rows: Option<u64>,
    chunk: Option<usize>,
    buffer_bytes: Option<usize>,
    tool_calls: Option<u64>,
    stdout_bytes: Option<usize>,
    allow_load: Option<bool>,
    allow_record: Option<bool>,
    allow_print: Option<bool>,
}
fn integer(v: &str) -> Result<i64, String> {
    v.parse().map_err(|_| "invalid i64 decimal".into())
}
fn dispatch(command: Command) -> Result<Value, String> {
    ENGINE.with(|engine| {
        let mut e = engine.borrow_mut();
        match command {
            Command::Prepare { source, inputs } => {
                if inputs.len() > 32 {
                    return Err("Input".into());
                }
                let inputs = inputs
                    .iter()
                    .map(|(n, t)| {
                        Ok((
                            n.as_str(),
                            match t.as_str() {
                                "i64" => Type::I64,
                                "bool" => Type::Bool,
                                _ => return Err("unknown input type".to_string()),
                            },
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                e.prepare(&source, &inputs)
            }
            Command::Create {
                unit,
                inputs,
                limits: o,
            } => {
                let mut l = ScriptLimits::default();
                if let Some(x) = o.rows {
                    l.limits.rows = x;
                }
                if let Some(x) = o.chunk {
                    l.limits.chunk = x;
                }
                if let Some(x) = o.buffer_bytes {
                    l.limits.buffer_bytes = x;
                }
                if let Some(x) = o.tool_calls {
                    l.tool_calls = x;
                }
                if let Some(x) = o.stdout_bytes {
                    l.stdout_bytes = x;
                }
                if let Some(x) = o.allow_load {
                    l.limits.allow_load = x;
                }
                if let Some(x) = o.allow_record {
                    l.limits.allow_record = x;
                }
                if let Some(x) = o.allow_print {
                    l.allow_print = x;
                }
                let inputs = inputs
                    .iter()
                    .map(|x| integer(x))
                    .collect::<Result<Vec<_>, _>>()?;
                e.create(unit, &inputs, l)
            }
            Command::Advance { task, count, trace } => e.advance(task, count, trace),
            Command::Snapshot { task } => e.snapshot(task),
            Command::Cancel { task } => e.cancel(task),
            Command::Complete {
                task,
                request,
                rows,
                price,
                quantity,
                receipt,
                error,
            } => {
                if error.is_none()
                    && let Some(count) = rows
                {
                    // This is a bounded trusted fixture provider, never script-selected networking.
                    let q = Quote {
                        unit_price_cents: integer(price.as_deref().unwrap_or("7"))?,
                        quantity: integer(quantity.as_deref().unwrap_or("3"))?,
                    };
                    return e.complete_fixture(task, request, count, q);
                }
                e.complete(
                    task,
                    request,
                    None,
                    receipt.as_deref().map(integer).transpose()?,
                    error,
                )
            }
            Command::Reset => {
                *e = Engine::default();
                Ok(json!({"reset":true}))
            }
        }
    })
}
/// Resize owned command storage, never accept an arbitrary pointer from JS.
#[unsafe(no_mangle)]
pub extern "C" fn command_buffer(len: usize) -> usize {
    if len > 65536 {
        return 0;
    }
    INPUT.with(|b| {
        let mut b = b.borrow_mut();
        b.resize(len, 0);
        b.as_mut_ptr() as usize
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn command_run() -> usize {
    let result = INPUT
        .with(|b| serde_json::from_slice::<Command>(&b.borrow()).map_err(|e| e.to_string()))
        .and_then(dispatch);
    let value = match result {
        Ok(v) => json!({"ok":true,"value":v}),
        Err(e) => json!({"ok":false,"error":e}),
    };
    OUTPUT.with(|b| {
        let mut b = b.borrow_mut();
        *b = serde_json::to_vec(&value).expect("JSON values serialize");
        b.len()
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn result_buffer() -> usize {
    OUTPUT.with(|b| b.borrow().as_ptr() as usize)
}

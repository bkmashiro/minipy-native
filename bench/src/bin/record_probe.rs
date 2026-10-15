//! Bounded end-to-end Quote/v1 versus Tool record scan probe.
//! Usage: cargo run --release --offline --manifest-path bench/Cargo.toml --features record-probe --bin record-probe -- [reps] [rows]
use minipy_native::{
    buffer::{Quote, QuoteBuffer, RecordBuffer},
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    ir::Type,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
    tool::{RecordField, RecordSchema, Reply, ResultSchema, ToolSchema},
};
use std::{rc::Rc, sync::Arc, time::Instant};
const QUOTE: &str = "rows = tools.load_quotes(seed)\ntotal = 0\nfor r in rows:\n    if r.quantity > 0:\n        total += r.unit_price_cents * r.quantity\nprint(total)";
const RECORD: &str = "rows = tools.fetch_rows(seed)\ntotal = 0\nfor r in rows:\n    if r.quantity > 0:\n        total += r.price * r.quantity\nprint(total)";
fn schema() -> RecordSchema {
    RecordSchema {
        stride: 16,
        fields: vec![
            RecordField {
                name: "quantity".into(),
                ty: Type::I64,
                offset: 8,
            },
            RecordField {
                name: "price".into(),
                ty: Type::I64,
                offset: 0,
            },
        ],
    }
}
fn tools() -> [ToolSchema; 1] {
    [ToolSchema {
        id: 43,
        name: "fetch_rows".into(),
        arguments: vec!["seed".into()],
        result: ResultSchema::Records(schema()),
    }]
}
struct Source {
    rows: Vec<Quote>,
    wire: Vec<u8>,
}
impl Provider for Source {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        QuoteBuffer::from_quotes(self.rows.clone())
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
    fn allows_tool(&self, id: u32) -> bool {
        id == 43
    }
    async fn call_tool(&self, _: u32, _: [i64; 2], _: usize) -> Result<Reply, TaskError> {
        Ok(Reply::Records(RecordBuffer::import_bytes(
            schema(),
            &self.wire,
        )?))
    }
}
struct Prepared {
    quote_native: Rc<CodeUnit>,
    record_native: Rc<CodeUnit>,
    quote_tape: Rc<InterpreterUnit>,
    record_tape: Rc<InterpreterUnit>,
}
impl Prepared {
    fn new() -> Self {
        let inputs = &[("seed", Type::I64)];
        Self {
            quote_native: Rc::new(CodeUnit::compile_script(QUOTE, inputs).unwrap()),
            record_native: Rc::new(CodeUnit::compile_with_tools(RECORD, inputs, &tools()).unwrap()),
            quote_tape: Rc::new(InterpreterUnit::prepare_script(QUOTE, inputs).unwrap()),
            record_tape: Rc::new(
                InterpreterUnit::prepare_with_tools(RECORD, inputs, &tools()).unwrap(),
            ),
        }
    }
}
async fn once(
    engine: &str,
    warm: bool,
    prepared: &Prepared,
    provider: Arc<Source>,
) -> (u128, String) {
    let start = Instant::now();
    let inputs = &[("seed", Type::I64)];
    let result = match engine {
        "quote-native" => {
            let code = if warm {
                prepared.quote_native.clone()
            } else {
                Rc::new(CodeUnit::compile_script(QUOTE, inputs).unwrap())
            };
            runtime::execute_script(
                code,
                provider,
                &[7],
                ScriptLimits::default(),
                Cancellation::default(),
                None,
            )
            .await
        }
        "record-native" => {
            let code = if warm {
                prepared.record_native.clone()
            } else {
                Rc::new(CodeUnit::compile_with_tools(RECORD, inputs, &tools()).unwrap())
            };
            runtime::execute_script(
                code,
                provider,
                &[7],
                ScriptLimits::default(),
                Cancellation::default(),
                None,
            )
            .await
        }
        "quote-tape" => {
            let unit = if warm {
                prepared.quote_tape.clone()
            } else {
                Rc::new(InterpreterUnit::prepare_script(QUOTE, inputs).unwrap())
            };
            interpreter::execute_script(
                unit,
                provider,
                &[7],
                ScriptLimits::default(),
                Cancellation::default(),
                None,
            )
            .await
        }
        "record-tape" => {
            let unit = if warm {
                prepared.record_tape.clone()
            } else {
                Rc::new(InterpreterUnit::prepare_with_tools(RECORD, inputs, &tools()).unwrap())
            };
            interpreter::execute_script(
                unit,
                provider,
                &[7],
                ScriptLimits::default(),
                Cancellation::default(),
                None,
            )
            .await
        }
        _ => unreachable!(),
    };
    result.status.unwrap();
    (start.elapsed().as_nanos(), result.stdout)
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut args = std::env::args().skip(1);
    let reps: usize = args.next().unwrap_or_else(|| "15".into()).parse().unwrap();
    let count: usize = args
        .next()
        .unwrap_or_else(|| "4096".into())
        .parse()
        .unwrap();
    assert!((1..=100_000).contains(&count) && (1..=100).contains(&reps));
    let rows: Vec<Quote> = (0..count)
        .map(|i| Quote {
            unit_price_cents: i as i64 % 17,
            quantity: i as i64 % 5 - 1,
        })
        .collect();
    let wire = rows
        .iter()
        .flat_map(|r| {
            r.unit_price_cents
                .to_le_bytes()
                .into_iter()
                .chain(r.quantity.to_le_bytes())
        })
        .collect();
    let expected = format!(
        "{}\n",
        rows.iter()
            .filter(|r| r.quantity > 0)
            .map(|r| r.unit_price_cents * r.quantity)
            .sum::<i64>()
    );
    let provider = Arc::new(Source { rows, wire });
    let prepared = Prepared::new();
    println!(
        "{}",
        serde_json::json!({"kind":"identity","reps":reps,"rows":count,"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,"source_quote":QUOTE,"source_record":RECORD,"wire":"little-endian contiguous i64 pairs; record import decodes; Quote provider clones source rows"})
    );
    let engines = ["quote-native", "record-native", "quote-tape", "record-tape"];
    for warm in [false, true] {
        for engine in engines {
            assert_eq!(
                once(engine, warm, &prepared, provider.clone()).await.1,
                expected
            );
        }
        for i in 0..reps {
            let order = if i % 2 == 0 {
                engines
            } else {
                [engines[3], engines[2], engines[1], engines[0]]
            };
            for engine in order {
                let (ns, stdout) = once(engine, warm, &prepared, provider.clone()).await;
                assert_eq!(stdout, expected);
                println!(
                    "{}",
                    serde_json::json!({"kind":"sample","mode":if warm {"warm"} else {"cold"},"engine":engine,"index":i,"ns":ns})
                );
            }
        }
    }
}

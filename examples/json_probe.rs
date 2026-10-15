//! Small valid-input parser A/B and real source/backend smoke. No trace in timing builds.
#[cfg(not(feature = "tool-json"))]
fn main() {
    eprintln!("requires --features tool-json");
}
#[cfg(feature = "tool-json")]
mod probe {
    use bytes::Bytes;
    use minipy_native::{
        buffer::{Quote, QuoteBuffer},
        codegen::CodeUnit,
        error::TaskError,
        interpreter::{self, InterpreterUnit},
        ir::Type,
        provider::Provider,
        runtime::{self, Cancellation, ScriptLimits},
        tool::{Reply, ResultSchema, ToolSchema},
    };
    use serde::Deserialize;
    use std::{hint::black_box, rc::Rc, sync::Arc, time::Instant};
    #[derive(Deserialize)]
    struct Fields {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        a: i64,
        b: i64,
    }
    #[derive(Deserialize)]
    struct Row {
        price: i64,
        quantity: i64,
    }
    struct P {
        bytes: Bytes,
        quotes: QuoteBuffer,
    }
    impl Provider for P {
        async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
            Ok(self.quotes.clone())
        }
        async fn record_total(&self, batch: i64, total: i64) -> Result<i64, TaskError> {
            total.checked_add(batch).ok_or(TaskError::Overflow)
        }
        fn allows_tool(&self, id: u32) -> bool {
            id == 2
        }
        async fn call_tool(&self, _: u32, _: [i64; 2], _: usize) -> Result<Reply, TaskError> {
            Ok(Reply::Json(self.bytes.clone()))
        }
    }
    fn scalar(raw: &[u8], path: &[&str]) -> i64 {
        let lv = sonic_rs::get(raw, path).unwrap();
        sonic_rs::from_slice(lv.as_raw_str().as_bytes()).unwrap()
    }
    fn parse(arm: &str, work: &str, raw: &[u8]) -> i64 {
        if work == "array_1000" {
            match arm {
                "sonic_lazy" => {
                    let mut sum = 0;
                    for row in sonic_rs::to_array_iter(raw) {
                        let row = row.unwrap();
                        let r = row.as_raw_str().as_bytes();
                        sum += scalar(r, &["price"]) * scalar(r, &["quantity"]);
                    }
                    sum
                }
                "sonic_row_slots" => {
                    // One array cursor, one object cursor per row, two reused scalar
                    // field slots. Conversion happens only after encountering a key.
                    let mut slots = [None; 2];
                    let mut sum = 0;
                    for row in sonic_rs::to_array_iter(raw) {
                        slots.fill(None);
                        let row = row.unwrap();
                        for field in sonic_rs::to_object_iter(row.as_raw_str()) {
                            let (key, value) = field.unwrap();
                            let slot = match key.as_ref() {
                                "price" => 0,
                                "quantity" => 1,
                                _ => continue,
                            };
                            if slots[slot].is_none() {
                                slots[slot] =
                                    Some(sonic_rs::from_str::<i64>(value.as_raw_str()).unwrap());
                            }
                            if slots.iter().all(Option::is_some) {
                                break;
                            }
                        }
                        sum += slots[0].unwrap() * slots[1].unwrap();
                    }
                    sum
                }
                "sonic_typed" => sonic_rs::from_slice::<Vec<Row>>(raw)
                    .unwrap()
                    .iter()
                    .map(|r| r.price * r.quantity)
                    .sum(),
                "serde_typed" => serde_json::from_slice::<Vec<Row>>(raw)
                    .unwrap()
                    .iter()
                    .map(|r| r.price * r.quantity)
                    .sum(),
                "serde_dom" => serde_json::from_slice::<serde_json::Value>(raw)
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| r["price"].as_i64().unwrap() * r["quantity"].as_i64().unwrap())
                    .sum(),
                _ => unreachable!(),
            }
        } else {
            let repetitions = if work == "repeated_16" { 16 } else { 1 };
            match arm {
                "sonic_lazy" => {
                    let mut sum = 0;
                    for _ in 0..repetitions {
                        sum += scalar(raw, &["data", "a"]) + scalar(raw, &["data", "b"]);
                    }
                    sum
                }
                "sonic_typed" => {
                    let f = sonic_rs::from_slice::<Fields>(raw).unwrap();
                    (f.data.a + f.data.b) * repetitions
                }
                "serde_typed" => {
                    let f = serde_json::from_slice::<Fields>(raw).unwrap();
                    (f.data.a + f.data.b) * repetitions
                }
                "serde_dom" => {
                    let f = serde_json::from_slice::<serde_json::Value>(raw).unwrap();
                    (f["data"]["a"].as_i64().unwrap() + f["data"]["b"].as_i64().unwrap())
                        * repetitions
                }
                _ => unreachable!(),
            }
        }
    }
    fn sample(
        work: &str,
        arm: &str,
        lifecycle: &str,
        rep: usize,
        n: usize,
        memory: (usize, usize),
        mut f: impl FnMut() -> i64,
    ) {
        let (prepared, code) = memory;
        // One real warm-up/oracle invocation outside the measurement boundary.
        let expected = f();
        #[cfg(feature = "json-allocation-profile")]
        super::allocs::reset();
        let start = Instant::now();
        let mut sum = 0;
        for _ in 0..n {
            sum += black_box(f());
        }
        let elapsed = start.elapsed().as_nanos();
        #[cfg(feature = "json-allocation-profile")]
        let (mode, allocations, allocated) =
            ("allocation", super::allocs::calls(), super::allocs::bytes());
        #[cfg(not(feature = "json-allocation-profile"))]
        let (mode, allocations, allocated) = ("timing", String::new(), String::new());
        assert_eq!(sum, expected * n as i64);
        let prepared = if prepared > 0 {
            prepared.to_string()
        } else {
            String::new()
        };
        let code = if code > 0 {
            code.to_string()
        } else {
            String::new()
        };
        let copy = if arm.starts_with("minipy_") { "0" } else { "" };
        println!(
            "{mode},{work},{arm},{lifecycle},{rep},{n},{elapsed},{allocations},{allocated},{copy},{prepared},{code},{sum}"
        );
    }
    pub fn run() {
        let diagnostics = cfg!(feature = "json-allocation-profile");
        println!(
            "mode,workload,arm,lifecycle,rep,iterations,total_ns,allocation_calls,allocated_bytes,host_to_task_payload_copy_bytes,prepared_bytes,code_bytes,checksum"
        );
        let fields = serde_json::to_vec(
            &serde_json::json!({"data":{"a":17,"b":23},"padding":"x".repeat(8192)}),
        )
        .unwrap();
        let rows = serde_json::to_vec(
            &(0..1000)
                .map(|i| serde_json::json!({"price":i%97,"quantity":i%5}))
                .collect::<Vec<_>>(),
        )
        .unwrap();
        for rep in 0..if diagnostics { 1 } else { 3 } {
            for work in ["few_fields", "array_1000", "repeated_16"] {
                let raw = if work == "array_1000" { &rows } else { &fields };
                let expected = parse("serde_typed", work, raw);
                for arm in [
                    "sonic_lazy",
                    "sonic_row_slots",
                    "sonic_typed",
                    "serde_typed",
                    "serde_dom",
                ] {
                    if arm == "sonic_row_slots" && work != "array_1000" {
                        continue;
                    }
                    assert_eq!(parse(arm, work, raw), expected);
                    sample(
                        work,
                        arm,
                        "parse_run_cleanup",
                        rep,
                        if diagnostics {
                            1
                        } else if work == "array_1000" {
                            100
                        } else {
                            1000
                        },
                        (0, 0),
                        || parse(arm, work, black_box(raw)),
                    );
                }
            }
        }
        // This probe uses ready frozen providers, no sockets. Keep reactor controls
        // equal to the pre-HTTP baseline; actual HTTP uses the full runtime in tests.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let p = Arc::new(P {
            bytes: Bytes::from(fields),
            quotes: QuoteBuffer::from_quotes(
                (0..1000)
                    .map(|i| Quote {
                        unit_price_cents: i % 97,
                        quantity: i % 5,
                    })
                    .collect(),
            )
            .unwrap(),
        });
        let mut repeated = "p = tools.fetch(batch_id)\ntotal = 0\n".to_owned();
        for _ in 0..16 {
            repeated.push_str("total += p[\"data\"][\"a\"] + p[\"data\"][\"b\"]\n");
        }
        repeated.push_str("print(total)\n");
        let few = "p = tools.fetch(batch_id)\nprint(p[\"data\"][\"a\"] + p[\"data\"][\"b\"])\n";
        let sources = [
            ("json_few", few),
            ("json_repeated", repeated.as_str()),
            ("scalar", include_str!("../fixtures/scalar_script.py")),
            ("weighted", include_str!("../fixtures/weighted_script.py")),
            ("stateful", include_str!("../fixtures/common_stateful.py")),
        ];
        let schemas = [ToolSchema {
            id: 2,
            name: "fetch".into(),
            arguments: vec!["batch_id".into()],
            result: ResultSchema::Json(vec![
                vec!["data".into(), "a".into()],
                vec!["data".into(), "b".into()],
            ]),
        }];
        let inputs = &[("batch_id", Type::I64), ("price_limit", Type::I64)];
        for (work, source) in sources {
            for native in [false, true] {
                let arm = if native {
                    "minipy_native"
                } else {
                    "minipy_tape"
                };
                let compile_native = || {
                    if work.starts_with("json_") {
                        CodeUnit::compile_with_tools(source, inputs, &schemas).unwrap()
                    } else {
                        CodeUnit::compile_script(source, inputs).unwrap()
                    }
                };
                let compile_tape = || {
                    if work.starts_with("json_") {
                        InterpreterUnit::prepare_with_tools(source, inputs, &schemas).unwrap()
                    } else {
                        InterpreterUnit::prepare_script(source, inputs).unwrap()
                    }
                };
                let native_unit = native.then(|| Rc::new(compile_native()));
                let tape_unit = (!native).then(|| Rc::new(compile_tape()));
                let prepared = tape_unit.as_ref().map_or(0, |u| u.prepared_bytes());
                let code = native_unit.as_ref().map_or(0, |u| u.code_bytes);
                let mut expected = None;
                for rep in 0..if diagnostics { 1 } else { 3 } {
                    for lifecycle in ["source_cold", "prepared"] {
                        sample(
                            work,
                            arm,
                            lifecycle,
                            rep,
                            if diagnostics {
                                1
                            } else if lifecycle == "source_cold" {
                                20
                            } else {
                                100
                            },
                            (prepared, code),
                            || {
                                let outcome = runtime.block_on(async {
                                    if native {
                                        runtime::execute_script(
                                            if lifecycle == "source_cold" {
                                                Rc::new(compile_native())
                                            } else {
                                                native_unit.as_ref().unwrap().clone()
                                            },
                                            p.clone(),
                                            &[7, 700],
                                            ScriptLimits::default(),
                                            Cancellation::default(),
                                            None,
                                        )
                                        .await
                                    } else {
                                        interpreter::execute_script(
                                            if lifecycle == "source_cold" {
                                                Rc::new(compile_tape())
                                            } else {
                                                tape_unit.as_ref().unwrap().clone()
                                            },
                                            p.clone(),
                                            &[7, 700],
                                            ScriptLimits::default(),
                                            Cancellation::default(),
                                            None,
                                        )
                                        .await
                                    }
                                });
                                assert_eq!(outcome.status, Ok(()));
                                if let Some(expected) = &expected {
                                    assert_eq!(&outcome.stdout, expected);
                                } else {
                                    expected = Some(outcome.stdout.clone());
                                }
                                outcome.stdout.bytes().map(i64::from).sum()
                            },
                        );
                    }
                }
            }
        }
    }
}
#[cfg(feature = "json-allocation-profile")]
mod allocs {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
    };
    thread_local! {static CALLS:Cell<usize>=const{Cell::new(0)};static BYTES:Cell<usize>=const{Cell::new(0)};}
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
#[cfg(feature = "tool-json")]
fn main() {
    probe::run()
}

//! G bounded three-engine experiment. argv: ENGINE CASE MODE ROWS REPS
#[path = "../v2_common.rs"]
mod common;
use common::{BATCH, LIMIT, RequestProvider};
use minipy_native::{
    codegen::CodeUnit,
    interpreter::{self, InterpreterUnit},
    ir::Type,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
};
use monty::{MontyRun, RunProgress};
use monty_types::{CompileOptions, ExtFunctionResult, MontyObject, PrintWriter, ResourceTracker};
use std::{
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
#[cfg(feature = "allocation-diagnostics")]
#[global_allocator]
static ALLOC: common::CountingAllocator = common::CountingAllocator;
const INPUTS: &[(&str, Type)] = &[("batch_id", Type::I64), ("price_limit", Type::I64)];
fn source(case: &str) -> String {
    match case {
        "scalar"=>include_str!("../../../fixtures/scalar_script.py").into(),
        "weighted"=>include_str!("../../../fixtures/weighted_script.py").into(),
        "stateful"=>"quotes=tools.load_quotes(batch_id)\ntotal=0\nprevious=0\nfor quote in quotes:\n    if quote.quantity > 0 and quote.unit_price_cents <= price_limit and quote.unit_price_cents >= previous:\n        amount=quote.unit_price_cents * quote.quantity\n        total += amount\n        previous=quote.unit_price_cents\nreceipt=tools.record_total(batch_id,total)\nprint(receipt)".into(),
        _=>panic!("case"),
    }
}
fn expected(case: &str, n: usize) -> String {
    if case == "scalar" {
        "734\nTrue\n".into()
    } else {
        format!(
            "{}\n",
            common::oracle(
                n,
                if case == "stateful" {
                    "stateful_branch"
                } else {
                    "weighted_scan"
                },
                0
            )
        )
    }
}
fn monty(source: &str, clear: bool) -> MontyRun {
    // Secondary strong baseline for these two known benchmark shapes only:
    // the Quote list has no future consumer at the final record. Do not infer
    // this from arbitrary user source or add a source-rewriting runtime path.
    let source = if clear {
        std::borrow::Cow::Owned(
            source
                .lines()
                .map(|line| {
                    if line.contains("tools.record_total(") {
                        format!("quotes = []\n{line}\n")
                    } else {
                        format!("{line}\n")
                    }
                })
                .collect::<String>(),
        )
    } else {
        std::borrow::Cow::Borrowed(source)
    };
    MontyRun::new(
        source
            .replace("tools.load_quotes(", "await load_quotes(")
            .replace("tools.record_total(", "await record_total(")
            .replace("quote.unit_price_cents", "quote[0]")
            .replace("quote.quantity", "quote[1]"),
        "g.py",
        vec![
            "batch_id".into(),
            "price_limit".into(),
            "load_quotes".into(),
            "record_total".into(),
        ],
        CompileOptions::default(),
    )
    .unwrap()
}
async fn monty_run(code: MontyRun, p: Arc<RequestProvider>) -> String {
    let mut stdout = String::new();
    let mut progress = code
        .start(
            vec![
                MontyObject::int(BATCH),
                MontyObject::int(LIMIT),
                MontyObject::function("load_quotes", None),
                MontyObject::function("record_total", None),
            ],
            ResourceTracker::default(),
            PrintWriter::CollectString(&mut stdout, Some(16384)),
        )
        .unwrap();
    loop {
        match progress {
            RunProgress::Complete(_) => return stdout,
            RunProgress::FunctionCall(call) => {
                let batch = call.args.arg(0).unwrap().as_int().unwrap();
                let total = call.args.arg(1).and_then(|x| x.as_int());
                let load = call.function_name == "load_quotes";
                assert!(load || call.function_name == "record_total");
                let id = call.call_id;
                let RunProgress::ResolveFutures(wait) = call
                    .resume_pending(PrintWriter::CollectString(&mut stdout, Some(16384)))
                    .unwrap()
                else {
                    panic!("actual await expected")
                };
                let (tx, rx) = tokio::sync::oneshot::channel();
                let pp = p.clone();
                tokio::spawn(async move {
                    let value = if load {
                        pp.load_quotes(batch).await.map(|b| common::object(&b))
                    } else {
                        pp.record_total(batch, total.unwrap())
                            .await
                            .map(MontyObject::int)
                    };
                    let _ = tx.send(value);
                });
                let value = rx.await.unwrap().unwrap();
                progress = wait
                    .resume(
                        vec![(id, ExtFunctionResult::Return(value))],
                        PrintWriter::CollectString(&mut stdout, Some(16384)),
                    )
                    .unwrap();
            }
            _ => panic!("unexpected progress"),
        }
    }
}
async fn native_run(code: Rc<CodeUnit>, p: Arc<RequestProvider>) -> String {
    let out = runtime::execute_script(
        code,
        p,
        &[BATCH, LIMIT],
        ScriptLimits::default(),
        Cancellation::default(),
        None,
    )
    .await;
    out.status.unwrap();
    out.stdout
}
async fn interpreted_run(code: Rc<InterpreterUnit>, p: Arc<RequestProvider>) -> String {
    let out = interpreter::execute_script(
        code,
        p,
        &[BATCH, LIMIT],
        ScriptLimits::default(),
        Cancellation::default(),
        None,
    )
    .await;
    out.status.unwrap();
    out.stdout
}
enum Prepared {
    Native(Rc<CodeUnit>),
    Interpreted(Rc<InterpreterUnit>),
    Monty(MontyRun),
}
fn prepare(engine: &str, source: &str) -> Prepared {
    match engine {
        "native" => Prepared::Native(Rc::new(CodeUnit::compile_script(source, INPUTS).unwrap())),
        "interpreter" => Prepared::Interpreted(Rc::new(
            InterpreterUnit::prepare_script(source, INPUTS).unwrap(),
        )),
        "monty" => Prepared::Monty(monty(source, false)),
        "monty-clear" => Prepared::Monty(monty(source, true)),
        _ => panic!("engine"),
    }
}
impl Prepared {
    fn duplicate(&self) -> Self {
        match self {
            Self::Native(c) => Self::Native(c.clone()),
            Self::Interpreted(c) => Self::Interpreted(c.clone()),
            Self::Monty(c) => Self::Monty(c.clone()),
        }
    }
    async fn run(self, p: Arc<RequestProvider>) -> String {
        match self {
            Self::Native(c) => native_run(c, p).await,
            Self::Interpreted(c) => interpreted_run(c, p).await,
            Self::Monty(c) => monty_run(c, p).await,
        }
    }
}
fn snapshot(engine: &str, case: &str, n: usize, count: usize, phase: &str) {
    common::emit(
        &serde_json::json!({"kind":"hold","engine":engine,"case":case,"rows":n,"count":count,"phase":phase,"allocation":common::allocation(),"physical":common::physical(),"input_payload_bytes":if phase=="load_wait" {n*16*count} else {0}}),
    );
}
async fn hold(engine: &str, case: &str, source: &str, n: usize, count: usize) {
    assert!(cfg!(feature = "allocation-diagnostics") && case != "scalar" && count <= 64);
    let load = Arc::new(common::Gate::new());
    let record = Arc::new(common::Gate::new());
    snapshot(engine, case, n, count, "startup");
    let mut jobs = Vec::new();
    for _ in 0..count {
        let mut p = RequestProvider::ready(n, Duration::ZERO);
        p.load_gate = Some(load.clone());
        p.record_gate = Some(record.clone());
        let p = Arc::new(p);
        let expected = expected(case, n);
        // Distinct concrete task futures; no inactive Monty union retained by other engines.
        match prepare(engine, source) {
            Prepared::Native(c) => jobs.push(tokio::task::spawn_local(async move {
                assert_eq!(native_run(c, p).await, expected)
            })),
            Prepared::Interpreted(c) => jobs.push(tokio::task::spawn_local(async move {
                assert_eq!(interpreted_run(c, p).await, expected)
            })),
            Prepared::Monty(c) => jobs.push(tokio::task::spawn_local(async move {
                assert_eq!(monty_run(c, p).await, expected)
            })),
        }
    }
    common::barrier(&load, count).await.unwrap();
    snapshot(engine, case, n, count, "load_wait");
    load.release(count);
    common::barrier(&record, count).await.unwrap();
    snapshot(engine, case, n, count, "record_wait");
    record.release(count);
    for j in jobs {
        j.await.unwrap();
    }
    snapshot(engine, case, n, count, "reclaimed");
}
// Two synthetic mixes, 40 arrivals at 500 us intervals per 100 ms window.
// Latency starts at planned arrival, including compiler/CPU queueing.
const LIGHT: &[(&str, usize, usize)] = &[
    ("scalar", 0, 24),
    ("weighted", 100, 8),
    ("stateful", 100, 4),
    ("weighted", 10000, 2),
    ("stateful", 100000, 2),
];
const SCAN: &[(&str, usize, usize)] = &[
    ("scalar", 0, 8),
    ("weighted", 100, 16),
    ("stateful", 100, 8),
    ("weighted", 10000, 4),
    ("stateful", 100000, 4),
];
async fn mixed(engine: &str, mix: &str, mode: &str, reps: usize) {
    let shapes = match mix {
        "light" => LIGHT,
        "scan" => SCAN,
        _ => panic!("mix"),
    };
    let delay_us: u64 = std::env::var("MINIPY_TOOL_DELAY_US")
        .unwrap_or_else(|_| "0".into())
        .parse()
        .unwrap();
    let hold = mode == "mixed-hold";
    assert_eq!(cfg!(feature = "allocation-diagnostics"), hold);
    let sources: Vec<_> = shapes.iter().map(|(case, _, _)| source(case)).collect();
    let expected_outputs: Vec<_> = shapes
        .iter()
        .map(|(case, n, _)| Rc::<str>::from(expected(case, *n)))
        .collect();
    let memory_snapshot = |phase: &str| {
        common::emit(&serde_json::json!({
        "kind":"hold","engine":engine,"mix":mix,"count":40,"phase":phase,
        "allocation":common::allocation(),"physical":common::physical(),
        "input_payload_bytes":if phase == "load_wait" {shapes.iter().map(|(_, n, c)| n*c*16).sum::<usize>()} else {0}}))
    };
    let warm: Vec<_> = if mode == "mixed-warm" {
        sources.iter().map(|s| prepare(engine, s)).collect()
    } else {
        Vec::new()
    };
    for sample in 0..reps {
        let load = Arc::new(common::Gate::new());
        let record = Arc::new(common::Gate::new());
        if hold {
            memory_snapshot("startup");
        }
        let cpu = common::cpu_ns();
        let start = Instant::now();
        let arrival_start = tokio::time::Instant::now();
        let deadline = arrival_start + Duration::from_millis(100);
        let mut jobs = Vec::new();
        // A coprime permutation intersperses long scans with short arrivals.
        let schedule: Vec<_> = shapes
            .iter()
            .enumerate()
            .flat_map(|(i, (_, _, count))| std::iter::repeat_n(i, *count))
            .collect();
        let waits: usize = shapes
            .iter()
            .filter(|(c, _, _)| *c != "scalar")
            .map(|(_, _, n)| n)
            .sum();
        for arrival in 0..40 {
            let shape = schedule[(arrival * 17) % 40];
            let (case, n, _) = shapes[shape];
            let s = sources[shape].clone();
            let expected_output = expected_outputs[shape].clone();
            let prepared = warm.get(shape).map(Prepared::duplicate);
            let engine = engine.to_owned();
            let due = arrival_start + Duration::from_micros(arrival as u64 * 500);
            let offset_ns = arrival as u64 * 500_000;
            let mut p = RequestProvider::ready(n, Duration::from_micros(delay_us));
            if hold {
                p.delay = Duration::ZERO;
                p.load_gate = Some(load.clone());
                p.record_gate = Some(record.clone());
            }
            let p = Arc::new(p);
            macro_rules! launch {
                ($variant:ident, $compile:expr, $run:ident) => {{
                    let code = prepared.map(|c| match c {
                        Prepared::$variant(c) => c,
                        _ => unreachable!(),
                    });
                    jobs.push(tokio::task::spawn_local(async move {
                        if !hold {
                            tokio::time::sleep_until(due).await;
                        }
                        let prep_start = Instant::now();
                        let code = code.unwrap_or_else(|| ($compile)(&s));
                        let prepare_ns = common::now_ns(prep_start);
                        let result = $run(code, p).await;
                        let latency_ns =
                            common::now_ns(start).saturating_sub(if hold { 0 } else { offset_ns });
                        assert_eq!(result, expected_output.as_ref());
                        (case, n, prepare_ns, latency_ns)
                    }));
                }};
            }
            match engine.as_str() {
                "native" => launch!(
                    Native,
                    |s: &str| Rc::new(CodeUnit::compile_script(s, INPUTS).unwrap()),
                    native_run
                ),
                "interpreter" => launch!(
                    Interpreted,
                    |s: &str| Rc::new(InterpreterUnit::prepare_script(s, INPUTS).unwrap()),
                    interpreted_run
                ),
                "monty" | "monty-clear" => launch!(
                    Monty,
                    |s: &str| monty(s, engine == "monty-clear"),
                    monty_run
                ),
                _ => unreachable!(),
            }
        }
        if hold {
            common::barrier(&load, waits).await.unwrap();
            memory_snapshot("load_wait");
            load.release(waits);
            common::barrier(&record, waits).await.unwrap();
            memory_snapshot("record_wait");
            record.release(waits);
        }
        let mut rows = Vec::new();
        for j in jobs {
            rows.push(j.await.unwrap());
        }
        let wall_ns = common::now_ns(start);
        let cpu_ns = common::cpu_ns() - cpu;
        if hold {
            memory_snapshot("reclaimed");
        } else {
            for (case, n, prepare_ns, latency_ns) in rows {
                common::emit(&serde_json::json!({"kind":"mixed","engine":engine,
                    "mix":mix,"delay_us":delay_us,"mode":mode,"sample":sample,"case":case,"rows":n,
                    "prepare_ns":prepare_ns,"latency_ns":latency_ns,
                    "batch_wall_ns":wall_ns,"batch_cpu_ns":cpu_ns,"overloaded":wall_ns>100_000_000,"completed":true}));
            }
            tokio::time::sleep_until(deadline).await;
        }
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 6, "engine case cold|warm|hold rows reps");
    let (engine, case, mode) = (&args[1], &args[2], &args[3]);
    let n: usize = args[4].parse().unwrap();
    let reps: usize = args[5].parse().unwrap();
    assert!(reps > 0 && n <= 100000);
    if mode.starts_with("mixed-") {
        assert!(["mixed-cold", "mixed-warm", "mixed-hold"].contains(&mode.as_str()));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        tokio::task::LocalSet::new().block_on(&rt, mixed(engine, case, mode, reps));
        return;
    }
    let source = source(case);
    let expected = expected(case, n);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    if mode == "hold" {
        tokio::task::LocalSet::new().block_on(&rt, hold(engine, case, &source, n, reps));
        return;
    }
    assert!(
        !cfg!(feature = "allocation-diagnostics"),
        "speed binary must be uninstrumented"
    );
    assert!(mode == "cold" || mode == "warm");
    let prepared_bytes = if engine == "interpreter" {
        Some(
            InterpreterUnit::prepare_script(&source, INPUTS)
                .unwrap()
                .prepared_bytes(),
        )
    } else {
        None
    };
    let warm = if mode == "warm" {
        Some(prepare(engine, &source))
    } else {
        None
    };
    for sample in 0..reps {
        let cpu = common::cpu_ns();
        let start = Instant::now();
        let p = Arc::new(RequestProvider::ready(n, Duration::ZERO));
        let producer_setup_ns = common::now_ns(start);
        let prep_start = Instant::now();
        let cold = if warm.is_none() {
            Some(prepare(engine, &source))
        } else {
            None
        };
        let prepare_ns = common::now_ns(prep_start);
        let run_start = Instant::now();
        let prepared = if let Some(cold) = cold {
            cold
        } else {
            warm.as_ref().unwrap().duplicate()
        };
        let result = rt.block_on(prepared.run(p));
        let execute_ns = common::now_ns(run_start);
        // The owning cold executable is consumed and reclaimed inside execute_ns.
        let drop_ns = 0;
        let wall_ns = common::now_ns(start);
        let cpu_ns = common::cpu_ns() - cpu;
        assert_eq!(result, expected);
        common::emit(
            &serde_json::json!({"kind":"speed","engine":engine,"case":case,"rows":n,"mode":mode,"sample":sample,"wall_ns":wall_ns,"cpu_ns":cpu_ns,"prepare_ns":prepare_ns,"execute_ns":execute_ns,"drop_ns":drop_ns,"producer_setup_ns":producer_setup_ns,"prepared_bytes":prepared_bytes,"completed":true}),
        );
    }
}

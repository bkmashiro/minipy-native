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
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 6, "engine case cold|warm|hold rows reps");
    let (engine, case, mode) = (&args[1], &args[2], &args[3]);
    let n: usize = args[4].parse().unwrap();
    let reps: usize = args[5].parse().unwrap();
    assert!(reps > 0 && n <= 10000);
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

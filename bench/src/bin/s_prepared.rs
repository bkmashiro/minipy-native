//! S retained prepared-owner diagnostic; no execution timing or runtime edits.
#[path = "../v2_common.rs"]
mod common;
use minipy_native::{codegen::CodeUnit, interpreter::InterpreterUnit, ir::Type};
use monty::MontyRun;
use monty_types::CompileOptions;
use std::{rc::Rc, time::Instant};
#[global_allocator]
static ALLOC: common::CountingAllocator = common::CountingAllocator;
const INPUTS: &[(&str, Type)] = &[("batch_id", Type::I64), ("price_limit", Type::I64)];
fn measure<T>(
    engine: &str,
    case: &str,
    count: usize,
    shared: bool,
    mut prepare: impl FnMut() -> T,
    size: impl Fn(&T) -> Option<usize>,
) {
    let snap = |phase: &str, logical: Option<usize>, prepare_ns: u64| {
        common::emit(&serde_json::json!({
            "kind":"prepared-owner", "engine":engine, "case":case, "count":count,
            "code":if shared {"shared"} else {"unique"}, "phase":phase,
            "allocation":common::allocation(), "physical":common::physical(),
            "logical_unit_bytes":logical, "prepare_ns":prepare_ns,
            "scope":"whole process and Rust requested allocations, including compiler scratch retained by runtime; logical bytes exclude mapping rounding and allocator/Rc overhead"
        }))
    };
    snap("startup", None, 0);
    let start = Instant::now();
    let seed = shared.then(|| Rc::new(prepare()));
    let owners: Vec<Rc<T>> = (0..count)
        .map(|_| seed.as_ref().map_or_else(|| Rc::new(prepare()), Rc::clone))
        .collect();
    let elapsed = start.elapsed().as_nanos() as u64;
    let logical = size(&owners[0]);
    snap("retained", logical, elapsed);
    drop(owners);
    drop(seed);
    snap("reclaimed", logical, elapsed);
}
fn main() {
    assert!(cfg!(feature = "allocation-diagnostics"));
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 5);
    let (engine, case, count, code) = (
        &args[1],
        &args[2],
        args[3].parse::<usize>().unwrap(),
        &args[4],
    );
    assert!((1..=32).contains(&count) && ["unique", "shared"].contains(&code.as_str()));
    let source = match case.as_str() {
        "scalar" => include_str!("../../../fixtures/common_scalar.py"),
        "weighted" => include_str!("../../../fixtures/common_weighted.py"),
        "stateful" => include_str!("../../../fixtures/common_stateful.py"),
        _ => panic!("case"),
    };
    match engine.as_str() {
        "native" => measure(
            engine,
            case,
            count,
            code == "shared",
            || CodeUnit::compile_script(source, INPUTS).unwrap(),
            |c| Some(c.code_bytes),
        ),
        "interpreter" => measure(
            engine,
            case,
            count,
            code == "shared",
            || InterpreterUnit::prepare_script(source, INPUTS).unwrap(),
            |c| Some(c.prepared_bytes()),
        ),
        "monty" => measure(
            engine,
            case,
            count,
            code == "shared",
            || {
                MontyRun::new(
                    source.into(),
                    "common.py",
                    vec!["batch_id".into(), "price_limit".into(), "tools".into()],
                    CompileOptions::default(),
                )
                .unwrap()
            },
            |_| None,
        ),
        _ => panic!("engine"),
    }
}

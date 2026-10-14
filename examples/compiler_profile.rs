//! Diagnostic-only driver; normal release measures the same public compile API.
//! compiler_profile CASE REPS [inventory-directory]
use minipy_native::{
    buffer::{Quote, QuoteBuffer},
    codegen::CodeUnit,
    error::TaskError,
    ir::Type,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
};
use std::{rc::Rc, sync::Arc, time::Instant};

#[cfg(feature = "compiler-profile")]
mod passes {
    use cranelift_codegen::timing::{self, Pass, Profiler};
    use std::{any::Any, cell::RefCell, rc::Rc, time::Instant};
    #[derive(Default, Clone, Copy)]
    pub struct Time {
        pub calls: u64,
        pub inclusive: u64,
        pub exclusive: u64,
    }
    #[derive(Default)]
    struct State {
        times: Vec<Time>,
        children: Vec<u64>,
    }
    struct Recorder(Rc<RefCell<State>>);
    struct Token {
        state: Rc<RefCell<State>>,
        pass: usize,
        start: Instant,
    }
    impl Profiler for Recorder {
        fn start_pass(&self, pass: Pass) -> Box<dyn Any> {
            self.0.borrow_mut().children.push(0);
            Box::new(Token {
                state: self.0.clone(),
                pass: pass as usize,
                start: Instant::now(),
            })
        }
    }
    impl Drop for Token {
        fn drop(&mut self) {
            let duration = self.start.elapsed().as_nanos() as u64;
            let mut s = self.state.borrow_mut();
            let child = s.children.pop().expect("LIFO timing");
            let t = &mut s.times[self.pass];
            t.calls += 1;
            t.inclusive += duration;
            t.exclusive += duration.checked_sub(child).expect("child <= parent");
            if let Some(parent) = s.children.last_mut() {
                *parent += duration;
            }
        }
    }
    pub struct Session(Rc<RefCell<State>>);
    impl Session {
        pub fn new() -> Self {
            let state = Rc::new(RefCell::new(State {
                times: vec![Time::default(); timing::NUM_PASSES],
                children: Vec::new(),
            }));
            timing::set_thread_profiler(Box::new(Recorder(state.clone())));
            Self(state)
        }
        pub fn take(&self) -> Vec<Time> {
            let mut s = self.0.borrow_mut();
            assert!(s.children.is_empty());
            std::mem::replace(&mut s.times, vec![Time::default(); timing::NUM_PASSES])
        }
    }
    // Public enum variants from the pinned 0.128.3 API, no unsafe enum conversion.
    pub const NAMES: [Pass; timing::NUM_PASSES] = [
        Pass::process_file,
        Pass::parse_text,
        Pass::wasm_translate_module,
        Pass::wasm_translate_function,
        Pass::verifier,
        Pass::compile,
        Pass::try_incremental_cache,
        Pass::store_incremental_cache,
        Pass::flowgraph,
        Pass::domtree,
        Pass::loop_analysis,
        Pass::preopt,
        Pass::egraph,
        Pass::gvn,
        Pass::licm,
        Pass::unreachable_code,
        Pass::remove_constant_phis,
        Pass::vcode_lower,
        Pass::vcode_emit,
        Pass::vcode_emit_finish,
        Pass::regalloc,
        Pass::regalloc_checker,
        Pass::layout_renumber,
        Pass::canonicalize_nans,
    ];
}

struct Ready;
fn rows() -> Vec<Quote> {
    (0..100)
        .map(|i| Quote {
            unit_price_cents: (i * 17 % 1000) as i64,
            quantity: (i % 7) as i64 - 2,
        })
        .collect()
}
impl Provider for Ready {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        // A real Tokio scheduling boundary, with no millisecond timer floor.
        tokio::task::yield_now().await;
        QuoteBuffer::from_quotes(rows())
    }
    async fn record_total(&self, batch: i64, total: i64) -> Result<i64, TaskError> {
        tokio::task::yield_now().await;
        total.checked_add(batch).ok_or(TaskError::Overflow)
    }
}
fn compile(source: &str) -> CodeUnit {
    CodeUnit::compile_script(
        source,
        &[("batch_id", Type::I64), ("price_limit", Type::I64)],
    )
    .unwrap()
}
fn validate(code: CodeUnit, case: &str) {
    let weighted: i64 = rows()
        .iter()
        .filter(|q| q.quantity > 0 && q.unit_price_cents <= 700)
        .map(|q| q.quantity * q.unit_price_cents)
        .sum();
    let first: i64 = rows()
        .iter()
        .filter(|q| q.quantity > 0)
        .map(|q| q.quantity)
        .sum();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&rt, async {
        if case == "legacy" {
            let out = runtime::execute(
                Rc::new(code),
                Arc::new(Ready),
                7,
                700,
                runtime::Limits::default(),
                Cancellation::default(),
                None,
            )
            .await
            .unwrap();
            assert_eq!(out, weighted + 7);
        } else {
            let out = runtime::execute_script(
                Rc::new(code),
                Arc::new(Ready),
                &[7, 700],
                ScriptLimits::default(),
                Cancellation::default(),
                None,
            )
            .await;
            out.status.unwrap();
            let expected = match case {
                "empty" => String::new(),
                "scalar" => "714\nTrue\n".into(),
                "scan" => format!("{weighted}\n"),
                "weighted" => format!("{}\n", weighted + 7),
                "two_scan" => format!("{}\n{}\n", first + weighted, first + 7),
                _ => unreachable!(),
            };
            assert_eq!(out.stdout, expected);
        }
    });
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(
        (3..=4).contains(&args.len()),
        "CASE REPS [inventory-directory]"
    );
    let case = &args[1];
    let reps: usize = args[2].parse().unwrap();
    assert!((1..=1000).contains(&reps));
    let file = match case.as_str() {
        "empty" => None,
        "scalar" => Some("scalar_script.py"),
        "scan" => Some("scan_script.py"),
        "weighted" => Some("weighted_script.py"),
        "two_scan" => Some("profile_two_scan.py"),
        "legacy" => Some("weighted_scan.py"),
        _ => panic!("unknown case"),
    };
    let base = std::env::var_os("MINIPY_FIXTURES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures"));
    let source = file
        .map(|f| std::fs::read_to_string(base.join(f)).unwrap())
        .unwrap_or_default();
    // No preparatory compile: sample zero observes initial scratch/backend state.
    // Every following sample reparses and regenerates code. No source/result cache.
    #[cfg(feature = "compiler-profile")]
    let session = passes::Session::new();
    for sample in 0..=reps {
        #[cfg(feature = "compiler-profile")]
        minipy_native::compiler_profile::reset(false);
        let start = Instant::now();
        let code = compile(&source);
        let compile_ns = start.elapsed().as_nanos() as u64;
        let code_bytes = code.code_bytes;
        assert!(code.clif.is_empty());
        let start = Instant::now();
        drop(code);
        let drop_ns = start.elapsed().as_nanos() as u64;
        print!(
            "{{\"kind\":\"compile\",\"case\":\"{case}\",\"sample\":{sample},\"diagnostic\":{},\"compile_ns\":{compile_ns},\"drop_ns\":{drop_ns},\"code_bytes\":{code_bytes}",
            cfg!(feature = "compiler-profile")
        );
        #[cfg(feature = "compiler-profile")]
        {
            let p = minipy_native::compiler_profile::take();
            print!(
                ",\"frontend_ns\":{},\"parse_ns\":{},\"type_ns\":{},\"setup_ns\":{},\"lowering_ns\":{},\"backend_ns\":{},\"finalize_ns\":{},\"cleanup_ns\":{}",
                p.frontend_ns,
                p.parse_ns,
                p.type_ns,
                p.setup_ns,
                p.lowering_ns,
                p.backend_ns,
                p.finalize_ns,
                p.cleanup_ns
            );
            print!(",\"passes\":[");
            let mut comma = "";
            for (i, t) in session
                .take()
                .iter()
                .enumerate()
                .filter(|(_, t)| t.calls != 0)
            {
                print!(
                    "{comma}{{\"name\":\"{:?}\",\"calls\":{},\"inclusive_ns\":{},\"exclusive_ns\":{}}}",
                    passes::NAMES[i],
                    t.calls,
                    t.inclusive,
                    t.exclusive
                );
                comma = ",";
            }
            print!("]");
        }
        println!("}}");
    }
    // Correctness executes the actual generated machine code outside all timings.
    validate(compile(&source), case);
    #[cfg(feature = "compiler-profile")]
    if let Some(dir) = args.get(3) {
        use minipy_native::compiler_profile as diag;
        diag::reset(true);
        let code = compile(&source);
        let p = diag::take();
        std::fs::create_dir_all(dir).unwrap();
        for (when, s) in [("before", &p.before), ("after", &p.after)] {
            std::fs::write(format!("{dir}/{case}-{when}.clif"), &s.text).unwrap();
            println!(
                "{{\"kind\":\"inventory\",\"case\":\"{case}\",\"when\":\"{when}\",\"blocks\":{},\"instructions\":{},\"block_params\":{},\"loads\":{},\"stores\":{},\"calls\":{},\"scalar_slots\":{},\"buffer_slots\":{},\"resume_states\":{},\"code_bytes\":{}}}",
                s.blocks,
                s.instructions,
                s.block_params,
                s.loads,
                s.stores,
                s.calls,
                p.scalar_slots,
                p.buffer_slots,
                p.resume_states,
                code.code_bytes
            );
        }
        validate(code, case);
    }
}

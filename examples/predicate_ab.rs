//! Bounded F A/B driver: CASE cold|prepared ROWS REPS [inventory-directory].
//! Plain build for timings; compiler-profile only for separate IR inventories.
use minipy_native::{
    buffer::{Quote, QuoteBuffer},
    codegen::CodeUnit,
    error::TaskError,
    ir::Type,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
};
use std::{rc::Rc, sync::Arc, time::Instant};

// clock_gettime ABI on the two supported 64-bit measurement hosts.
#[repr(C)]
struct Timespec {
    seconds: i64,
    nanos: i64,
}
unsafe extern "C" {
    fn clock_gettime(clock: i32, time: *mut Timespec) -> i32;
}
fn cpu_ns() -> u64 {
    #[cfg(target_os = "linux")]
    const PROCESS: i32 = 2;
    #[cfg(target_os = "macos")]
    const PROCESS: i32 = 12;
    let mut time = Timespec {
        seconds: 0,
        nanos: 0,
    };
    assert_eq!(unsafe { clock_gettime(PROCESS, &mut time) }, 0);
    time.seconds as u64 * 1_000_000_000 + time.nanos as u64
}
fn rows(n: usize) -> Vec<Quote> {
    (0..n)
        .map(|i| Quote {
            unit_price_cents: (i * 17 % 1000) as i64,
            quantity: (i % 7) as i64 - 2,
        })
        .collect()
}
struct Ready(usize);
impl Provider for Ready {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        tokio::task::yield_now().await;
        QuoteBuffer::from_quotes(rows(self.0))
    }
    async fn record_total(&self, batch: i64, total: i64) -> Result<i64, TaskError> {
        tokio::task::yield_now().await;
        total.checked_add(batch).ok_or(TaskError::Overflow)
    }
}
fn compile(source: &str) -> Rc<CodeUnit> {
    Rc::new(
        CodeUnit::compile_script(
            source,
            &[("batch_id", Type::I64), ("price_limit", Type::I64)],
        )
        .unwrap(),
    )
}
fn expected(case: &str, n: usize) -> String {
    if case == "scalar" {
        return "714\nTrue\n".into();
    }
    let mut total = 0i64;
    let mut previous = 0;
    for q in rows(n) {
        let price = q.unit_price_cents;
        let quantity = q.quantity;
        if case == "weighted" {
            if quantity > 0 && price <= 700 {
                total = total
                    .checked_add(price.checked_mul(quantity).unwrap())
                    .unwrap();
            }
        } else if !(quantity <= 0 || (price > 700 && previous > 0)) {
            if (price >= previous && quantity > 1) || previous <= 0 {
                total = total
                    .checked_add(price.checked_mul(quantity).unwrap())
                    .unwrap();
            } else {
                total = total.checked_sub(quantity).unwrap();
            }
        } else {
            total = total.checked_add(1).unwrap();
        }
        previous = price;
    }
    format!("{}\n", total.checked_add(7).unwrap())
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(
        (5..=6).contains(&args.len()),
        "CASE cold|prepared ROWS REPS [inventory-directory]"
    );
    let case = &args[1];
    let mode = &args[2];
    assert!(["cold", "prepared"].contains(&mode.as_str()));
    let n: usize = args[3].parse().unwrap();
    let reps: usize = args[4].parse().unwrap();
    assert!(n <= 10_000 && (1..=1000).contains(&reps));
    let source = match case.as_str() {
        "scalar" => include_str!("../fixtures/scalar_script.py"),
        "weighted" => include_str!("../fixtures/weighted_script.py"),
        "nested" => include_str!("../fixtures/predicate_nested.py"),
        _ => panic!("case"),
    };
    let oracle = expected(case, n);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let prepared = (mode == "prepared").then(|| compile(source));
    for sample in 0..=reps {
        let cpu = cpu_ns();
        let start = Instant::now();
        // Fresh owning source and compiler on every cold request, no source/code cache.
        let request = (mode == "cold").then(|| source.to_owned());
        let build = Instant::now();
        let code = prepared
            .clone()
            .unwrap_or_else(|| compile(request.as_deref().unwrap()));
        let compile_ns = if mode == "cold" {
            build.elapsed().as_nanos() as u64
        } else {
            0
        };
        let code_bytes = code.code_bytes;
        assert!(code.clif.is_empty());
        let out = rt.block_on(runtime::execute_script(
            code,
            Arc::new(Ready(n)),
            &[7, 700],
            ScriptLimits::default(),
            Cancellation::default(),
            None,
        ));
        drop(request);
        let wall_ns = start.elapsed().as_nanos() as u64;
        let cpu_ns = cpu_ns() - cpu;
        // Actual compiled result and status validated outside all measured intervals.
        assert_eq!(out.status, Ok(()));
        assert_eq!(out.stdout, oracle);
        println!(
            "{{\"kind\":\"sample\",\"case\":\"{case}\",\"mode\":\"{mode}\",\"rows\":{n},\"sample\":{sample},\"wall_ns\":{wall_ns},\"cpu_ns\":{cpu_ns},\"compile_ns\":{compile_ns},\"code_bytes\":{code_bytes},\"ok\":true}}"
        );
    }
    #[cfg(feature = "compiler-profile")]
    if let Some(dir) = args.get(5) {
        use minipy_native::compiler_profile as diag;
        diag::reset(true);
        let code = compile(source);
        let p = diag::take();
        std::fs::create_dir_all(dir).unwrap();
        for (when, s) in [("before", &p.before), ("after", &p.after)] {
            std::fs::write(format!("{dir}/{case}-{when}.clif"), &s.text).unwrap();
            let conversions = s.text.lines().filter(|l| l.contains("uextend")).count();
            println!(
                "{{\"kind\":\"inventory\",\"case\":\"{case}\",\"when\":\"{when}\",\"blocks\":{},\"instructions\":{},\"block_params\":{},\"uextend\":{conversions},\"resume_states\":{},\"code_bytes\":{}}}",
                s.blocks, s.instructions, s.block_params, p.resume_states, code.code_bytes
            );
        }
    }
}

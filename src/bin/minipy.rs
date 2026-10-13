//! Small real-source demonstration. Data is a provider fixture; computation is generated from the file.
use minipy_native::{
    buffer::Quote,
    codegen::CodeUnit,
    provider::TestProvider,
    runtime::{self, Cancellation, Limits, Trace},
};
use std::{rc::Rc, sync::Arc, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let source = match args.next() {
        Some(path) => std::fs::read_to_string(path)?,
        None => include_str!("../../fixtures/weighted_scan.py").to_owned(),
    };
    let show_ir = args.next().is_some_and(|arg| arg == "--ir");
    let start = Instant::now();
    let code = Rc::new(if show_ir {
        CodeUnit::compile_with_ir(&source, "speed")?
    } else {
        CodeUnit::compile(&source)?
    });
    println!(
        "compile_ns={} code_bytes={}",
        start.elapsed().as_nanos(),
        code.code_bytes
    );
    if show_ir {
        println!("{}", code.clif);
        #[cfg(feature = "compile-timing")]
        eprintln!("{}", cranelift_codegen::timing::take_current());
    }
    let provider = Arc::new(TestProvider::new(
        vec![
            Quote {
                unit_price_cents: 12,
                quantity: 3,
            },
            Quote {
                unit_price_cents: 800,
                quantity: 1,
            },
            Quote {
                unit_price_cents: -1,
                quantity: 2,
            },
        ],
        minipy_native::provider::Delays {
            load: std::time::Duration::from_millis(1),
            record: std::time::Duration::from_millis(1),
        },
    ));
    let trace: Trace = Rc::default();
    let executor = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()?;
    if code.is_legacy() {
        let receipt = executor.block_on(runtime::execute(
            code,
            provider.clone(),
            7,
            700,
            Limits::default(),
            Cancellation::default(),
            Some(trace.clone()),
        ))?;
        println!(
            "receipt={receipt} loads={} records={}",
            provider.load_count(),
            provider.record_count()
        );
    } else {
        let outcome = executor.block_on(runtime::execute_script(
            code,
            provider.clone(),
            &[7, 700],
            runtime::ScriptLimits::default(),
            Cancellation::default(),
            Some(trace.clone()),
        ));
        print!("{}", outcome.stdout);
        outcome.status?;
        println!(
            "status=ok loads={} records={}",
            provider.load_count(),
            provider.record_count()
        );
    }
    for e in trace.borrow().iter() {
        println!(
            "{} buffer_bytes={} frame_bytes={} remaining={} index={}",
            e.phase, e.buffer_bytes, e.frame_bytes, e.remaining, e.index
        );
    }
    Ok(())
}

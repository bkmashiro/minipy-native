#[cfg(feature = "compiler-profile")]
fn main() {
    use minipy_native::{codegen::CodeUnit, compiler_profile, ir::Type};
    for (case, source) in [
        ("scalar", include_str!("../fixtures/scalar_script.py")),
        ("weighted", include_str!("../fixtures/weighted_script.py")),
    ] {
        compiler_profile::reset(true);
        let unit = CodeUnit::compile_script(
            source,
            &[("batch_id", Type::I64), ("price_limit", Type::I64)],
        )
        .unwrap();
        let p = compiler_profile::take();
        println!(
            "CASE {case} bytes={} resumes={} before_blocks={} after_blocks={} before_loads={} after_loads={}\nBEFORE\n{}\nAFTER\n{}",
            unit.code_bytes,
            p.resume_states,
            p.before.blocks,
            p.after.blocks,
            p.before.loads,
            p.after.loads,
            p.before.text,
            p.after.text
        );
    }
}
#[cfg(not(feature = "compiler-profile"))]
fn main() {
    panic!("requires compiler-profile");
}

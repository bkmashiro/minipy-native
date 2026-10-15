//! Isolated feasibility gate. Raw-code install is an optimistic cost floor,
//! NOT a source compiler, E2E backend, or equivalent sandbox comparison.
#![cfg(all(target_arch = "aarch64", target_os = "macos"))]
#[path = "../evidence/cb-copy-patch/templates.rs"]
mod templates;
use cranelift_codegen::{
    ir::{AbiParam, types},
    settings,
};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use minipy_native::{
    buffer::QuoteBuffer,
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    ir::Type,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
};
use std::{rc::Rc, sync::Arc, time::Instant};

// Trusted fixed-slot, leaf C ABI, never an arbitrary guest address.
type Entry = unsafe extern "C" fn(*mut i64) -> i32;
struct Installed {
    module: Option<JITModule>,
    entry: Entry,
}
impl Drop for Installed {
    fn drop(&mut self) {
        if let Some(module) = self.module.take() {
            // No function pointer escapes this owner; every synchronous call ended.
            unsafe { module.free_memory() }
        }
    }
}
impl Installed {
    fn new(code: &[u8]) -> Result<Self, String> {
        let isa = cranelift_native::builder()
            .map_err(|e| e.to_string())?
            .finish(settings::Flags::new(settings::builder()))
            .map_err(|e| e.to_string())?;
        let mut module = JITModule::new(JITBuilder::with_isa(
            isa,
            cranelift_module::default_libcall_names(),
        ));
        // Own memory even if declaration/finalization fails; normal JITModule Drop leaks.
        let mut unit = Self {
            module: None,
            entry: uninstalled,
        };
        let result: Result<Entry, String> = (|| {
            let mut sig = module.make_signature();
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I32));
            let id = module
                .declare_function("cb_region", Linkage::Local, &sig)
                .map_err(|e| e.to_string())?;
            module
                .define_function_bytes(id, 4, code, &[])
                .map_err(|e| e.to_string())?;
            module.finalize_definitions().map_err(|e| e.to_string())?;
            let ptr = module.get_finalized_function(id);
            // All bytes come from validated compiler templates with checked local patches.
            Ok(unsafe { std::mem::transmute::<*const u8, Entry>(ptr) })
        })();
        unit.module = Some(module);
        unit.entry = result?;
        Ok(unit)
    }
    fn run(&self, state: &mut [i64; 4]) -> i32 {
        // Compiled functions only address slots 1..=3 and never retain the pointer.
        unsafe { (self.entry)(state.as_mut_ptr()) }
    }
}
unsafe extern "C" fn uninstalled(_: *mut i64) -> i32 {
    2
}

fn patch_branch(code: &mut [u8], at: usize, target: usize) -> Result<(), &'static str> {
    if !at.is_multiple_of(4)
        || !target.is_multiple_of(4)
        || at.checked_add(4).is_none_or(|end| end > code.len())
        || target >= code.len()
    {
        return Err("unaligned/out-of-bounds branch");
    }
    let word = u32::from_le_bytes(code[at..at + 4].try_into().unwrap());
    if word != 0x14000000 {
        return Err("not the compiler B relocation");
    }
    let delta = (target as i128 - at as i128) / 4;
    if !(-(1i128 << 25)..(1i128 << 25)).contains(&delta) {
        return Err("BR26 range");
    }
    let patched = word | ((delta as u32) & 0x03ffffff);
    code[at..at + 4].copy_from_slice(&patched.to_le_bytes());
    Ok(())
}
fn stitch(parts: &[(&[u8], usize)]) -> Result<Vec<u8>, &'static str> {
    // This gate has no source admission. Explicit private bound prevents giant copies.
    if parts.is_empty() || parts.len() > 16 {
        return Err("gate template bound");
    }
    let mut code = Vec::new();
    let mut patches = Vec::new();
    for (bytes, hole) in parts {
        patches.push(code.len() + hole);
        code.extend_from_slice(bytes);
    }
    code.extend_from_slice(templates::DONE);
    for (i, at) in patches.into_iter().enumerate() {
        let target: usize = parts[..=i].iter().map(|(bytes, _)| bytes.len()).sum();
        patch_branch(&mut code, at, target)?;
    }
    Ok(code)
}

#[test]
fn real_templates_arithmetic_and_error_exit() {
    let values = [
        i64::MIN,
        i64::MIN + 1,
        -100,
        -2,
        -1,
        0,
        1,
        2,
        100,
        i64::MAX - 1,
        i64::MAX,
    ];
    let items = [
        (templates::ADD, templates::ADD_NEXT, 0),
        (templates::SUB, templates::SUB_NEXT, 1),
        (templates::MUL, templates::MUL_NEXT, 2),
    ];
    for (blob, next, op) in items {
        let bytes = stitch(&[(blob, next)]).unwrap();
        let unit = Installed::new(&bytes).unwrap();
        for &a in &values {
            for &b in &values {
                let expected = match op {
                    0 => a.checked_add(b),
                    1 => a.checked_sub(b),
                    _ => a.checked_mul(b),
                };
                let mut state = [0, a, b, 12345];
                let status = unit.run(&mut state);
                assert_eq!(status, i32::from(expected.is_none()));
                assert_eq!(state[3], expected.unwrap_or(12345));
                assert_eq!(&state[1..3], &[a, b]);
            }
        }
    }
    // Add fails first. Later mul would succeed, and must never overwrite sentinel.
    let bytes = stitch(&[
        (templates::ADD, templates::ADD_NEXT),
        (templates::MUL, templates::MUL_NEXT),
    ])
    .unwrap();
    let mut state = [0, i64::MAX, 1, 12345];
    assert_eq!(Installed::new(&bytes).unwrap().run(&mut state), 1);
    assert_eq!(state[3], 12345);
    // A successful add followed by failing mul preserves the previous write.
    let mut state = [0, i64::MAX - 2, 2, 12345];
    assert_eq!(Installed::new(&bytes).unwrap().run(&mut state), 1);
    assert_eq!(state[3], i64::MAX);
}
#[test]
fn patch_rejects_invalid_metadata() {
    let mut code = vec![0; 8];
    assert!(patch_branch(&mut code, 0, 4).is_err());
    code[..4].copy_from_slice(&0x14000000u32.to_le_bytes());
    assert!(patch_branch(&mut code, 1, 4).is_err());
    assert!(patch_branch(&mut code, 0, 8).is_err());
    assert!(patch_branch(&mut code, 0, 3).is_err());
    assert!(stitch(&[]).is_err());
    let mut code = stitch(&[(templates::ADD, templates::ADD_NEXT)]).unwrap();
    assert!(patch_branch(&mut code, templates::ADD_NEXT, templates::ADD.len()).is_err());
}
struct Host;
impl Provider for Host {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        Err(TaskError::Permission)
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
}

#[test]
#[ignore = "serial release feasibility gate under shared perf lock"]
fn cost_gate() {
    let mode = std::env::var("CB_MODE").unwrap();
    let reps: usize = std::env::var("CB_REPS").unwrap().parse().unwrap();
    assert!(reps <= 256);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let host = Arc::new(Host);
    let mut rows = Vec::new();
    for sample in 0..=reps {
        // Input/source construction is outside every arm, no code/unit cache.
        let b = sample as i64 + 1;
        let source = format!("x = a + {b}\nprint(x)\n");
        let start = Instant::now();
        if mode == "install_floor" {
            // Optimistic preselected bytes: NO parser/type/lowering/output/runtime.
            // Includes ISA/module creation, stitch/patch/copy/install/call/free.
            let code = stitch(&[(templates::ADD, templates::ADD_NEXT)]).unwrap();
            let unit = Installed::new(&code).unwrap();
            let mut state = [0, 7, b, 0];
            assert_eq!(unit.run(&mut state), 0);
            assert_eq!(state[3], 7 + b);
            drop(unit);
            drop(code);
        } else {
            let inputs = &[("a", Type::I64)];
            let outcome = if mode == "native" {
                let unit = Rc::new(CodeUnit::compile_script(&source, inputs).unwrap());
                rt.block_on(runtime::execute_script(
                    unit,
                    host.clone(),
                    &[7],
                    ScriptLimits::default(),
                    Cancellation::default(),
                    None,
                ))
            } else {
                assert_eq!(mode, "tape");
                let unit = Rc::new(InterpreterUnit::prepare_script(&source, inputs).unwrap());
                rt.block_on(interpreter::execute_script(
                    unit,
                    host.clone(),
                    &[7],
                    ScriptLimits::default(),
                    Cancellation::default(),
                    None,
                ))
            };
            assert_eq!(outcome.status, Ok(()));
            assert_eq!(outcome.stdout, format!("{}\n", 7 + b));
            drop(outcome);
        }
        rows.push((sample, start.elapsed().as_nanos()));
    }
    for (sample, ns) in rows {
        println!("CB {{\"sample\":{sample},\"ns\":{ns}}}");
    }
}

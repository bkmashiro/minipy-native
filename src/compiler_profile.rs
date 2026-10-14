//! Opt-in compiler diagnostics. Not built into the default runtime.
use cranelift_codegen::ir::Function;
use std::{cell::RefCell, time::Instant};

#[derive(Debug, Default)]
pub struct IrSize {
    pub blocks: usize,
    pub instructions: usize,
    pub block_params: usize,
    pub loads: usize,
    pub stores: usize,
    pub calls: usize,
    pub text: String,
}
#[derive(Debug, Default)]
pub struct Profile {
    pub frontend_ns: u64,
    pub parse_ns: u64,
    pub type_ns: u64,
    pub setup_ns: u64,
    pub lowering_ns: u64,
    pub backend_ns: u64,
    pub finalize_ns: u64,
    pub cleanup_ns: u64,
    pub scalar_slots: usize,
    pub buffer_slots: usize,
    pub resume_states: usize,
    pub before: IrSize,
    pub after: IrSize,
}
thread_local! {
    static CURRENT: RefCell<Profile> = RefCell::new(Profile::default());
    // Expensive inventories are collected only in a separate, untimed compile.
    static INVENTORY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
pub fn reset(inventory: bool) {
    CURRENT.with(|p| *p.borrow_mut() = Profile::default());
    INVENTORY.with(|p| p.set(inventory));
}
pub fn take() -> Profile {
    CURRENT.with(|p| std::mem::take(&mut *p.borrow_mut()))
}
pub(crate) fn update(f: impl FnOnce(&mut Profile)) {
    CURRENT.with(|p| f(&mut p.borrow_mut()));
}
pub(crate) fn elapsed(start: Instant) -> u64 {
    start.elapsed().as_nanos() as u64
}
pub(crate) fn inventory(func: &Function, before: bool) {
    if !INVENTORY.with(|p| p.get()) {
        return;
    }
    let mut size = IrSize::default();
    for block in func.layout.blocks() {
        size.blocks += 1;
        size.block_params += func.dfg.block_params(block).len();
        for inst in func.layout.block_insts(block) {
            size.instructions += 1;
            match func.dfg.insts[inst].opcode().to_string().as_str() {
                "load" => size.loads += 1,
                "store" => size.stores += 1,
                "call" => size.calls += 1,
                _ => {}
            }
        }
    }
    size.text = func.display().to_string();
    update(|p| {
        if before {
            p.before = size
        } else {
            p.after = size
        }
    });
}

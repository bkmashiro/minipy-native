//! Diagnostic event counts, absent from ordinary builds. No inferred time shares.
use std::cell::RefCell;
#[derive(Clone, Debug, Default)]
pub struct Counts {
    pub expr: [u64; 7], // int, bool, local, field, not, binary, short-circuit
    pub ops: [u64; 10], // set, eval, return, print, branch, jump, start, scan, next, tool
    pub fuel_reservations: u64,
    pub yields: u64,
}
thread_local! { static COUNTS: RefCell<Counts> = RefCell::new(Counts::default()); }
pub fn reset() {
    COUNTS.with(|c| *c.borrow_mut() = Counts::default());
}
pub fn take() -> Counts {
    COUNTS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}
pub(crate) fn update(f: impl FnOnce(&mut Counts)) {
    COUNTS.with(|c| f(&mut c.borrow_mut()));
}

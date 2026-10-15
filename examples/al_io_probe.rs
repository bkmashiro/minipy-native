//! Bounded I/O scheduler qualification. No runtime or source semantics changed.
#[cfg(not(feature = "tool-json"))]
fn main() {
    eprintln!("requires --features tool-json");
}
#[cfg(feature = "tool-json")]
#[path = "al_io_probe/support.rs"]
mod support;
#[cfg(feature = "tool-json")]
fn main() {
    support::run();
}

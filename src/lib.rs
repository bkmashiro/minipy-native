pub mod buffer;
#[cfg(not(target_arch = "wasm32"))]
pub mod codegen;
#[cfg(feature = "compiler-profile")]
pub mod compiler_profile;
pub mod error;
#[cfg(feature = "fixture-reference")]
pub mod fixture_reference;
pub mod frontend;
pub mod interpreter;
#[cfg(feature = "interpreter-profile")]
pub mod interpreter_profile;
pub mod ir;
#[cfg(not(target_arch = "wasm32"))]
pub mod provider;
pub mod runtime;
#[cfg(all(target_arch = "wasm32", feature = "debug-runtime"))]
mod wasm;

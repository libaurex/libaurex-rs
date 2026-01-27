pub mod engine;
mod ffi;
pub mod enums;
mod singletons;
pub mod aurex;
mod structs;
mod decoding_loop;
pub mod extern_c_bindings;

uniffi::setup_scaffolding!();
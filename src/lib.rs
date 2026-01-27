pub mod engine;
mod ffi;
pub mod enums;
mod singletons;
pub mod aurex;
mod structs;
mod decoding_loop;

uniffi::setup_scaffolding!();
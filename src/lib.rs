pub mod engine;
pub mod enums;
mod singletons;
pub mod aurex;
mod structs;
mod decoding_loop;
pub mod dart_bindings;

uniffi::setup_scaffolding!();
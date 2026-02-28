// Library target exists solely for criterion benchmarks.
// The binary entry point is main.rs; this file re-declares the module tree so
// that bench harnesses can import types via `keydr::engine::*` / `keydr::session::*`.
// Most code is only exercised through the binary, so suppress dead_code warnings.
#![allow(dead_code)]

// Public: used by benchmarks and the generate_test_profiles binary
pub mod config;
pub mod engine;
pub mod keyboard;
pub mod session;
pub mod store;

// Private: required transitively by engine/session (won't compile without them)
mod app;
mod event;
mod generator;
mod ui;

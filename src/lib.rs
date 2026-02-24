// Library target exists solely for criterion benchmarks.
// The binary entry point is main.rs; this file re-declares the module tree so
// that bench harnesses can import types via `keydr::engine::*` / `keydr::session::*`.
// Most code is only exercised through the binary, so suppress dead_code warnings.
#![allow(dead_code)]

// Public: used directly by benchmarks
pub mod engine;
pub mod session;

// Private: required transitively by engine/session (won't compile without them)
mod app;
mod config;
mod event;
mod generator;
mod keyboard;
mod store;
mod ui;

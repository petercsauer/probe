//! WASM plugin system for PRB using Extism.
//!
//! This crate provides the host-side WASM runtime for loading and executing
//! protocol decoder plugins compiled to WebAssembly.

pub mod adapter;
pub mod error;
pub mod loader;
pub mod runtime;

pub use adapter::{WasmDecoderFactory, WasmProtocolDetector};
pub use error::PluginError;
pub use loader::WasmPluginLoader;
pub use runtime::WasmLimits;

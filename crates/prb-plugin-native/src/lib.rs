//! Native plugin loader for PRB.
//!
//! This crate provides the infrastructure to load and execute native (shared library)
//! plugins that implement the PRB plugin API.

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]

mod adapter;
mod error;
mod loader;

pub use adapter::{NativeDecoderFactory, NativeProtocolDetector};
pub use error::PluginError;
pub use loader::{LoadedPlugin, NativePluginLoader};

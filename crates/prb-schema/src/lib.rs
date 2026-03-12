//! Protobuf schema registry for PRB.
//!
//! This crate provides schema loading, storage, and resolution for protobuf message types.
//! It supports both pre-compiled descriptor sets (.desc files) and runtime compilation of
//! .proto files via protox.

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
#![allow(missing_docs)] // TODO: Complete documentation in future segment

mod error;
mod registry;

pub use error::{Result, SchemaError};
pub use registry::SchemaRegistry;

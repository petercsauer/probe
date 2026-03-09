//! MCAP-backed storage layer for DebugEvent sessions.
//!
//! This crate provides persistent storage for DebugEvents using the MCAP format,
//! enabling session-based analysis of captured protocol traffic.

pub mod error;
pub mod metadata;
pub mod reader;
pub mod writer;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod cli_tests;

pub use error::{Result, StorageError};
pub use metadata::SessionMetadata;
pub use reader::{ChannelInfo, SessionReader};
pub use writer::SessionWriter;

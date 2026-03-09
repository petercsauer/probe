//! JSON fixture file adapter for PRB.
//!
//! This crate provides a CaptureAdapter implementation that reads debug events
//! from JSON fixture files for testing and offline analysis.

pub mod adapter;
pub mod error;
pub mod format;

pub use adapter::JsonFixtureAdapter;
pub use error::FixtureError;
pub use format::{FixtureEvent, FixtureFile, FixtureSource};

//! Test utilities for the probe project.
//!
//! This crate provides centralized test fixtures and builders for `DebugEvent`
//! to eliminate duplication across test files.
//!
//! # Examples
//!
//! ```
//! use prb_test_utils::{event, grpc_event, event_builder};
//! use prb_core::TransportKind;
//!
//! // Use a preset fixture
//! let evt = grpc_event(42);
//!
//! // Customize via builder
//! let custom = event_builder()
//!     .transport(TransportKind::Zmq)
//!     .build();
//! ```

mod builders;
mod fixtures;

pub use builders::*;
pub use fixtures::*;

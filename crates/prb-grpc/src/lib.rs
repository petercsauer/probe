#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]
#![allow(clippy::single_match_else)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::trivially_copy_pass_by_ref)]
//! gRPC/HTTP2 protocol decoder for offline PCAP analysis.
//!
//! This crate implements gRPC protocol decoding from reassembled TCP streams,
//! including HTTP/2 frame parsing, HPACK header decompression, gRPC message
//! extraction with compression support, trailer/status parsing, and graceful
//! degradation for mid-stream captures.

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]

mod correlation;
mod decoder;
mod error;
mod h2;
mod lpm;

pub use correlation::GrpcCorrelationStrategy;
pub use decoder::GrpcDecoder;
pub use error::GrpcError;

#[cfg(test)]
mod tests;

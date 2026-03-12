#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]
#![allow(clippy::single_match_else)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::trivially_copy_pass_by_ref)]
//! ZMTP wire protocol decoder for offline PCAP analysis.
//!
//! This crate implements `ZeroMQ` ZMTP 3.0/3.1 protocol decoding from reassembled
//! TCP streams, including greeting/handshake parsing, multipart message reassembly,
//! metadata extraction (socket type, identity), and mid-stream graceful degradation.

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
#![allow(missing_docs)] // TODO: Complete documentation in future segment

mod correlation;
mod decoder;
mod error;
mod parser;

pub use correlation::ZmqCorrelationStrategy;
pub use decoder::ZmqDecoder;
pub use error::ZmqError;

#[cfg(test)]
mod tests;

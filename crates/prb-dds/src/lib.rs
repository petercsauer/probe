#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]
#![allow(clippy::single_match_else)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::trivially_copy_pass_by_ref)]
//! DDS/RTPS protocol decoder for offline PCAP analysis.
//!
//! This crate implements DDS/RTPS protocol decoding from UDP datagrams,
//! including RTPS message parsing, DATA submessage payload extraction,
//! SEDP discovery tracking for topic name resolution, and GUID-based
//! correlation metadata.

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]

mod correlation;
mod decoder;
mod discovery;
mod error;
mod rtps_parser;

pub use correlation::DdsCorrelationStrategy;
pub use decoder::DdsDecoder;
pub use error::DdsError;

#[cfg(test)]
mod tests;

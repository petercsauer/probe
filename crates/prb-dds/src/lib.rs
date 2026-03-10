//! DDS/RTPS protocol decoder for offline PCAP analysis.
//!
//! This crate implements DDS/RTPS protocol decoding from UDP datagrams,
//! including RTPS message parsing, DATA submessage payload extraction,
//! SEDP discovery tracking for topic name resolution, and GUID-based
//! correlation metadata.

mod decoder;
mod discovery;
mod error;

pub use decoder::DdsDecoder;
pub use error::DdsError;

#[cfg(test)]
mod tests;

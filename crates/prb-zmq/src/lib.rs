//! ZMTP wire protocol decoder for offline PCAP analysis.
//!
//! This crate implements ZeroMQ ZMTP 3.0/3.1 protocol decoding from reassembled
//! TCP streams, including greeting/handshake parsing, multipart message reassembly,
//! metadata extraction (socket type, identity), and mid-stream graceful degradation.

mod correlation;
mod decoder;
mod error;
mod parser;

pub use correlation::ZmqCorrelationStrategy;
pub use decoder::ZmqDecoder;
pub use error::ZmqError;

#[cfg(test)]
mod tests;

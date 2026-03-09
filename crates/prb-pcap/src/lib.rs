//! PCAP/pcapng file reading and TLS key extraction for PRB.
//!
//! This crate provides transparent reading of both legacy PCAP and modern pcapng
//! capture formats, with support for extracting embedded TLS keys from pcapng
//! Decryption Secrets Blocks (DSB).

mod error;
mod reader;

pub use error::PcapError;
pub use reader::{PcapFileReader, TlsKeyStore};

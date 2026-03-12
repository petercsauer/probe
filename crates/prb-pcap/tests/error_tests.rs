//! Tests for error type construction and conversion.

use prb_core::CoreError;
use prb_pcap::PcapError;

#[test]
fn test_pcap_error_construction() {
    // Test Parse error variant
    let err = PcapError::Parse("test parse error".to_string());
    assert!(err.to_string().contains("test parse error"));

    // Test UnsupportedFormat error variant
    let err = PcapError::UnsupportedFormat("unknown format".to_string());
    assert!(err.to_string().contains("unknown format"));

    // Test InvalidLinktype error variant
    let err = PcapError::InvalidLinktype("linktype 999".to_string());
    assert!(err.to_string().contains("linktype 999"));

    // Test TlsKey error variant
    let err = PcapError::TlsKey("key load failed".to_string());
    assert!(err.to_string().contains("key load failed"));
}

#[test]
fn test_pcap_error_from_io_error() {
    // Test conversion from std::io::Error
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let pcap_err: PcapError = io_err.into();
    assert!(pcap_err.to_string().contains("I/O error"));
}

#[test]
fn test_pcap_error_to_core_error() {
    // Test conversion to CoreError
    let pcap_err = PcapError::Parse("parse failure".to_string());
    let core_err: CoreError = pcap_err.into();

    match core_err {
        CoreError::PayloadDecode(msg) => {
            assert!(msg.contains("parse failure"));
        }
        _ => panic!("Expected PayloadDecode error"),
    }
}

#[test]
fn test_pcap_error_debug() {
    // Test Debug impl
    let err = PcapError::Parse("debug test".to_string());
    let debug_str = format!("{:?}", err);
    assert!(debug_str.contains("Parse"));
}

//! Real-world capture fixture validation tests.
//!
//! These tests verify that real-world capture fixtures exist and are accessible.
//! Full end-to-end pipeline testing is done in prb-pcap integration tests.

use std::fs::File;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/captures")
}

#[test]
fn test_real_data_http2_h2c_fixture() {
    let path = fixtures_dir().join("http2/http2-h2c.pcap");
    assert!(path.exists(), "HTTP/2 h2c fixture should exist");

    let file = File::open(&path).expect("Should open HTTP/2 fixture");
    let metadata = file.metadata().expect("Should read metadata");
    assert!(metadata.len() > 1000, "HTTP/2 fixture should have content");
}

#[test]
fn test_real_data_tcp_fixtures() {
    let fixtures = vec!["tcp/dns-remoteshell.pcap", "tcp/tcp-ecn-sample.pcap", "tcp/200722_tcp_anon.pcapng"];

    for fixture in fixtures {
        let path = fixtures_dir().join(fixture);
        if path.exists() {
            let file = File::open(&path).expect("Should open TCP fixture");
            let metadata = file.metadata().expect("Should read metadata");
            assert!(metadata.len() > 100, "Fixture {} should have content", fixture);
        }
    }
}

#[test]
fn test_real_data_tls_fixtures() {
    let fixtures = vec!["tls/tls12.pcapng", "tls/tls13.pcapng"];

    for fixture in fixtures {
        let path = fixtures_dir().join(fixture);
        if path.exists() {
            let file = File::open(&path).expect("Should open TLS fixture");
            let metadata = file.metadata().expect("Should read metadata");
            assert!(metadata.len() > 1000, "TLS fixture {} should have content", fixture);
        }
    }
}

#[test]
fn test_real_data_readme_documentation() {
    let readme_path = fixtures_dir().join("README.md");
    assert!(readme_path.exists(), "README.md should document fixtures");

    let content = std::fs::read_to_string(&readme_path).expect("Should read README");
    assert!(content.contains("HTTP/2"), "README should document HTTP/2");
    assert!(content.contains("Wireshark"), "README should credit sources");
    assert!(content.len() > 500, "README should be comprehensive");
}

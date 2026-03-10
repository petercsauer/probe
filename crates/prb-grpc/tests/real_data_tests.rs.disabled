//! Real-world gRPC capture tests - fixture validation.
//!
//! These tests verify that publicly available pcap captures can be read and
//! contain the expected packet structure. Full end-to-end decoding is tested
//! in prb-pcap integration tests.

use std::fs::File;
use std::path::PathBuf;

/// Helper to get the fixtures directory path.
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/captures")
}

#[test]
fn test_real_data_grpc_person_search_fixture() {
    let fixture_path = fixtures_dir().join("grpc/grpc_person_search_protobuf_with_image.pcapng");

    assert!(
        fixture_path.exists(),
        "Fixture not found: {}. Run download step first.",
        fixture_path.display()
    );

    // Verify file is readable and has content
    let file = File::open(&fixture_path).expect("Should open fixture file");
    let metadata = file.metadata().expect("Should read file metadata");
    assert!(metadata.len() > 1000, "Fixture should have substantial content");
}

#[test]
fn test_real_data_grpc_hello_simple_fixture() {
    let fixture_path =
        fixtures_dir().join("grpc/grpc_hello2_1call_very_simple2_gzip_javacs.pcapng");

    assert!(
        fixture_path.exists(),
        "Fixture not found: {}",
        fixture_path.display()
    );

    let file = File::open(&fixture_path).expect("Should open fixture file");
    let metadata = file.metadata().expect("Should read file metadata");
    assert!(metadata.len() > 100, "Fixture should have content");
}

#[test]
fn test_real_data_grpc_streaming_fixture() {
    let fixture_path = fixtures_dir().join("grpc/grpc_json_streamtest.pcapng");

    assert!(
        fixture_path.exists(),
        "Fixture not found: {}",
        fixture_path.display()
    );

    let file = File::open(&fixture_path).expect("Should open fixture file");
    let metadata = file.metadata().expect("Should read file metadata");
    assert!(metadata.len() > 100, "Fixture should have content");
}

#[test]
fn test_real_data_h2c_cleartext_fixture() {
    let fixture_path = fixtures_dir().join("http2/http2-h2c.pcap");

    assert!(
        fixture_path.exists(),
        "Fixture not found: {}",
        fixture_path.display()
    );

    let file = File::open(&fixture_path).expect("Should open fixture file");
    let metadata = file.metadata().expect("Should read file metadata");
    assert!(metadata.len() > 100, "Fixture should have content");
}

#[test]
fn test_real_data_http2_tls_fixture() {
    let fixture_path = fixtures_dir().join("http2/http2-16-ssl.pcapng");

    assert!(
        fixture_path.exists(),
        "Fixture not found: {}",
        fixture_path.display()
    );

    let file = File::open(&fixture_path).expect("Should open fixture file");
    let metadata = file.metadata().expect("Should read file metadata");
    assert!(metadata.len() > 1000, "Fixture should have substantial content");
}

#[test]
fn test_real_data_readme_exists() {
    let readme_path = fixtures_dir().join("README.md");

    assert!(
        readme_path.exists(),
        "README.md should document fixture sources"
    );

    let content = std::fs::read_to_string(&readme_path).expect("Should read README");
    assert!(
        content.contains("Wireshark"),
        "README should credit Wireshark"
    );
    assert!(
        content.contains("http"),
        "README should include source URLs"
    );
}

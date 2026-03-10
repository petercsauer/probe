//! Unit tests for CaptureConfig.

use prb_capture::CaptureConfig;

#[test]
fn test_default_config_has_sane_values() {
    let config = CaptureConfig::default();

    assert_eq!(config.interface, "");
    assert_eq!(config.bpf_filter, None);
    assert_eq!(config.snaplen, 65535, "should capture full packets");
    assert!(config.promisc, "promiscuous mode should be enabled by default");
    assert!(
        config.immediate_mode,
        "immediate mode should be enabled by default"
    );
    assert_eq!(
        config.buffer_size,
        16 * 1024 * 1024,
        "buffer size should be 16MB"
    );
    assert_eq!(config.timeout_ms, 1000, "timeout should be 1 second");
    assert_eq!(config.tls_keylog_path, None);
}

#[test]
fn test_config_builder_pattern() {
    let config = CaptureConfig::new("eth0")
        .with_filter("tcp port 443")
        .with_snaplen(1500)
        .with_promisc(false)
        .with_buffer_size(8 * 1024 * 1024);

    assert_eq!(config.interface, "eth0");
    assert_eq!(config.bpf_filter, Some("tcp port 443".to_string()));
    assert_eq!(config.snaplen, 1500);
    assert!(!config.promisc);
    assert_eq!(config.buffer_size, 8 * 1024 * 1024);
}

#[test]
fn test_config_with_tls_keylog() {
    let config = CaptureConfig::new("lo").with_tls_keylog("/tmp/keylog.txt");

    assert_eq!(config.interface, "lo");
    assert_eq!(
        config.tls_keylog_path,
        Some(std::path::PathBuf::from("/tmp/keylog.txt"))
    );
}

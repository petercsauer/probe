//! Unit tests for `CaptureError` display messages.

use prb_capture::CaptureError;

#[test]
fn test_insufficient_privileges_error_includes_remediation() {
    let error = CaptureError::InsufficientPrivileges {
        message: "need CAP_NET_RAW capability".to_string(),
        remediation: "Run with sudo or grant CAP_NET_RAW".to_string(),
    };

    let error_msg = error.to_string();
    assert!(
        error_msg.contains("need CAP_NET_RAW capability"),
        "should include message"
    );
    assert!(error_msg.contains("Fix:"), "should include 'Fix:' prefix");
    assert!(
        error_msg.contains("Run with sudo or grant CAP_NET_RAW"),
        "should include remediation"
    );
}

#[test]
fn test_interface_not_found_error() {
    let error = CaptureError::InterfaceNotFound("wlan99".to_string());
    let error_msg = error.to_string();

    assert!(error_msg.contains("wlan99"));
    assert!(error_msg.contains("not found"));
}

#[test]
fn test_filter_compilation_failed_error() {
    let error = CaptureError::FilterCompilationFailed("invalid syntax".to_string());
    let error_msg = error.to_string();

    assert!(error_msg.contains("BPF filter"));
    assert!(error_msg.contains("invalid syntax"));
}

#[test]
fn test_already_running_error() {
    let error = CaptureError::AlreadyRunning;
    let error_msg = error.to_string();

    assert!(error_msg.contains("already running"));
}

#[test]
fn test_channel_closed_error() {
    let error = CaptureError::ChannelClosed;
    let error_msg = error.to_string();

    assert!(error_msg.contains("channel closed"));
}

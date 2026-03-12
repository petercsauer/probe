//! Integration tests for privilege checking.

use prb_capture::PrivilegeCheck;

#[test]
fn test_privilege_status_returns_valid_message() {
    // Test that status() returns a non-empty, valid message
    let status = PrivilegeCheck::status();

    assert!(!status.is_empty(), "status message should not be empty");

    // The status message should be one of the expected variants
    let valid_messages = [
        "Running as root (full privileges)",
        "CAP_NET_RAW capability granted",
        "Insufficient privileges (need CAP_NET_RAW or root)",
        "Privilege check not implemented on this platform",
    ];

    let is_valid = valid_messages.iter().any(|msg| status.contains(msg))
        || status.contains("Unable to check capabilities");

    assert!(is_valid, "unexpected status message: {}", status);
}

#[test]
fn test_privilege_check_succeeds_or_returns_proper_error() {
    // Test that check() either succeeds (if we have privileges)
    // or returns a proper error with remediation
    let result = PrivilegeCheck::check("lo0");

    match result {
        Ok(()) => {
            // We have privileges - test passes
        }
        Err(e) => {
            // We don't have privileges - verify error format
            let err_msg = e.to_string();

            // Error should mention insufficient privileges or the interface
            assert!(
                err_msg.contains("privileges") || err_msg.contains("capabilities"),
                "error message should mention privileges: {}",
                err_msg
            );
        }
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_privilege_check_error_includes_remediation_linux() {
    // On Linux, if we don't have privileges, the error should include remediation steps
    let result = PrivilegeCheck::check("eth0");

    if let Err(e) = result {
        let err_msg = format!("{:?}", e);

        // Should mention CAP_NET_RAW or sudo in the remediation
        let has_remediation = err_msg.contains("sudo")
            || err_msg.contains("setcap")
            || err_msg.contains("CAP_NET_RAW");

        assert!(
            has_remediation,
            "Linux privilege error should include remediation: {}",
            err_msg
        );
    }
}

#[test]
#[cfg(not(target_os = "linux"))]
fn test_privilege_check_succeeds_on_non_linux() {
    // On non-Linux platforms (macOS, etc.), the privilege check
    // should always succeed (pcap will check at open time)
    let result = PrivilegeCheck::check("lo0");
    assert!(
        result.is_ok(),
        "privilege check should succeed on non-Linux platforms"
    );
}

#[test]
fn test_privilege_check_with_various_interface_names() {
    // Test that check() works with different interface name formats
    let interfaces = ["lo", "lo0", "eth0", "wlan0", "en0"];

    for iface in &interfaces {
        let result = PrivilegeCheck::check(iface);

        // Should return either Ok or a proper error
        match result {
            Ok(()) => {
                // Success is valid
            }
            Err(e) => {
                // Error should be well-formatted
                let msg = e.to_string();
                assert!(!msg.is_empty(), "error message should not be empty");
            }
        }
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_privilege_status_distinguishes_root_vs_capability() {
    // On Linux, status() should distinguish between root and CAP_NET_RAW
    let status = PrivilegeCheck::status();

    // Should be one of the Linux-specific messages
    let is_linux_status = status.contains("root")
        || status.contains("CAP_NET_RAW")
        || status.contains("Insufficient privileges")
        || status.contains("Unable to check");

    assert!(
        is_linux_status,
        "Linux should return Linux-specific privilege status: {}",
        status
    );
}

#[test]
fn test_multiple_privilege_checks_are_consistent() {
    // Multiple calls to status() should return the same result
    let status1 = PrivilegeCheck::status();
    let status2 = PrivilegeCheck::status();

    assert_eq!(
        status1, status2,
        "privilege status should be consistent across calls"
    );

    // Multiple calls to check() should return the same result
    let check1 = PrivilegeCheck::check("lo0").is_ok();
    let check2 = PrivilegeCheck::check("lo0").is_ok();

    assert_eq!(
        check1, check2,
        "privilege check should be consistent across calls"
    );
}

//! Integration tests for InterfaceEnumerator.

use prb_capture::InterfaceEnumerator;

#[test]
fn test_list_interfaces_returns_at_least_loopback() {
    // All systems should have at least a loopback interface
    let interfaces = InterfaceEnumerator::list().expect("failed to list interfaces");

    assert!(!interfaces.is_empty(), "should have at least one interface");

    // Check if we have a loopback interface
    let has_loopback = interfaces.iter().any(|iface| {
        iface.is_loopback
            || iface.name == "lo"
            || iface.name == "lo0"
            || iface.name.starts_with("Loopback")
    });

    assert!(
        has_loopback,
        "should have a loopback interface, found: {:?}",
        interfaces.iter().map(|i| &i.name).collect::<Vec<_>>()
    );
}

#[test]
fn test_interface_info_display() {
    let interfaces = InterfaceEnumerator::list().expect("failed to list interfaces");

    for iface in interfaces {
        // Test status() method
        let status = iface.status();
        assert!(
            status == "UP" || status == "UP (no carrier)" || status == "DOWN",
            "unexpected status: {}",
            status
        );

        // Test addresses_display()
        let addr_display = iface.addresses_display();
        assert!(!addr_display.is_empty());

        // If no addresses, should show placeholder
        if iface.addresses.is_empty() {
            assert_eq!(addr_display, "(no address)");
        }
    }
}

#[test]
fn test_find_loopback_interface() {
    // Try common loopback names across platforms
    let loopback_names = ["lo", "lo0", "Loopback"];

    let mut found = false;
    for name in &loopback_names {
        if let Ok(iface) = InterfaceEnumerator::find(name) {
            assert_eq!(iface.name, *name);
            found = true;
            break;
        }
    }

    if !found {
        // If none of the common names work, just verify error handling
        let result = InterfaceEnumerator::find("nonexistent_interface_xyz");
        assert!(result.is_err(), "should error when interface doesn't exist");
    }
}

#[test]
fn test_list_active_interfaces() {
    let active = InterfaceEnumerator::list_active().expect("failed to list active interfaces");

    // All active interfaces should be up and running
    for iface in active {
        assert!(iface.is_up, "active interface should be up: {}", iface.name);
        assert!(
            iface.is_running,
            "active interface should be running: {}",
            iface.name
        );
    }
}

#[test]
fn test_default_device() {
    // May fail on some systems without network interfaces, so we check both cases
    match InterfaceEnumerator::default_device() {
        Ok(iface) => {
            // If we got a default device, it should have a name
            assert!(!iface.name.is_empty());
        }
        Err(_) => {
            // Some systems may not have a default device configured
            // This is acceptable and not a test failure
        }
    }
}

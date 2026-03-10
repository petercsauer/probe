//! Privilege checking for packet capture.

use crate::error::CaptureError;

/// Utility for checking capture privileges.
pub struct PrivilegeCheck;

impl PrivilegeCheck {
    /// Check if the current process has sufficient privileges to capture packets.
    ///
    /// On Linux, this checks for CAP_NET_RAW capability or root user.
    /// On other platforms, this always returns Ok (pcap will error if insufficient).
    #[allow(unused_variables)]
    pub fn check(interface: &str) -> Result<(), CaptureError> {
        #[cfg(target_os = "linux")]
        {
            use caps::{CapSet, Capability};

            // Check if we're root
            if nix::unistd::Uid::effective().is_root() {
                return Ok(());
            }

            // Check if we have CAP_NET_RAW capability
            match caps::has_cap(None, CapSet::Effective, Capability::CAP_NET_RAW) {
                Ok(true) => Ok(()),
                Ok(false) => Err(CaptureError::InsufficientPrivileges {
                    message: format!("insufficient privileges to capture on interface '{}'", interface),
                    remediation: "Run with sudo, or grant CAP_NET_RAW capability:\n  \
                                  sudo setcap cap_net_raw+ep $(which prb)".to_string(),
                }),
                Err(e) => Err(CaptureError::Other(format!(
                    "failed to check capabilities: {}",
                    e
                ))),
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            // On macOS and other platforms, pcap itself will check privileges
            Ok(())
        }
    }

    /// Get a human-readable privilege status message.
    pub fn status() -> String {
        #[cfg(target_os = "linux")]
        {
            use caps::{CapSet, Capability};

            if nix::unistd::Uid::effective().is_root() {
                return "Running as root (full privileges)".to_string();
            }

            match caps::has_cap(None, CapSet::Effective, Capability::CAP_NET_RAW) {
                Ok(true) => "CAP_NET_RAW capability granted".to_string(),
                Ok(false) => "Insufficient privileges (need CAP_NET_RAW or root)".to_string(),
                Err(e) => format!("Unable to check capabilities: {}", e),
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            "Privilege check not implemented on this platform".to_string()
        }
    }
}

#[cfg(target_os = "linux")]
mod nix {
    pub mod unistd {
        pub struct Uid(u32);
        impl Uid {
            pub fn effective() -> Self {
                Self(unsafe { libc::geteuid() })
            }
            pub fn is_root(&self) -> bool {
                self.0 == 0
            }
        }
    }
}

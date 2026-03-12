//! Network interface enumeration and information.

use crate::error::CaptureError;
use std::net::IpAddr;

/// Information about a network interface.
#[derive(Debug, Clone)]
pub struct InterfaceInfo {
    /// Interface name (e.g., "eth0", "wlan0").
    pub name: String,

    /// Human-readable description.
    pub description: Option<String>,

    /// IP addresses assigned to this interface.
    pub addresses: Vec<IpAddr>,

    /// Interface is administratively up.
    pub is_up: bool,

    /// Interface is running (cable connected, etc.).
    pub is_running: bool,

    /// Interface is a loopback device.
    pub is_loopback: bool,

    /// Interface is a wireless device.
    pub is_wireless: bool,
}

impl InterfaceInfo {
    /// Convert from a pcap Device.
    pub(crate) fn from_device(device: pcap::Device) -> Self {
        // Extract addresses
        let addresses: Vec<IpAddr> = device.addresses.iter().map(|addr| addr.addr).collect();

        // Determine interface properties from flags
        let is_up = device.flags.is_up();
        let is_running = device.flags.is_running();
        let is_loopback = device.flags.is_loopback();

        // Heuristic: wireless interfaces often have "wlan", "wifi", "wireless" in name
        let is_wireless = device.name.contains("wlan")
            || device.name.contains("wifi")
            || device.name.contains("wireless")
            || device
                .desc
                .as_ref()
                .is_some_and(|d| {
                    d.to_lowercase().contains("wireless")
                        || d.to_lowercase().contains("wi-fi")
                        || d.to_lowercase().contains("802.11")
                });

        Self {
            name: device.name,
            description: device.desc,
            addresses,
            is_up,
            is_running,
            is_loopback,
            is_wireless,
        }
    }

    /// Get a status string for display.
    #[must_use] 
    pub const fn status(&self) -> &str {
        if self.is_up && self.is_running {
            "UP"
        } else if self.is_up {
            "UP (no carrier)"
        } else {
            "DOWN"
        }
    }

    /// Get a formatted address list for display.
    #[must_use] 
    pub fn addresses_display(&self) -> String {
        if self.addresses.is_empty() {
            "(no address)".to_string()
        } else {
            self.addresses
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        }
    }
}

/// Utility for enumerating and querying network interfaces.
pub struct InterfaceEnumerator;

impl InterfaceEnumerator {
    /// List all available network interfaces.
    pub fn list() -> Result<Vec<InterfaceInfo>, CaptureError> {
        let devices = pcap::Device::list()?;
        Ok(devices
            .into_iter()
            .map(InterfaceInfo::from_device)
            .collect())
    }

    /// Find a specific interface by name.
    pub fn find(name: &str) -> Result<InterfaceInfo, CaptureError> {
        Self::list()?
            .into_iter()
            .find(|i| i.name == name)
            .ok_or_else(|| CaptureError::InterfaceNotFound(name.to_string()))
    }

    /// Get the default capture interface.
    pub fn default_device() -> Result<InterfaceInfo, CaptureError> {
        let device = pcap::Device::lookup()?
            .ok_or_else(|| CaptureError::Other("no default capture device found".into()))?;
        Ok(InterfaceInfo::from_device(device))
    }

    /// List only active (up and running) interfaces.
    pub fn list_active() -> Result<Vec<InterfaceInfo>, CaptureError> {
        Ok(Self::list()?
            .into_iter()
            .filter(|i| i.is_up && i.is_running)
            .collect())
    }
}

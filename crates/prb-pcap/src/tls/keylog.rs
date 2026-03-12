//! SSLKEYLOGFILE parser and TLS key storage.
//!
//! Parses NSS Key Log Format used by Firefox, Chrome, and Wireshark:
//! - TLS 1.2: `CLIENT_RANDOM <hex> <hex_master_secret>`
//! - TLS 1.3: `CLIENT_TRAFFIC_SECRET_0 <hex> <hex_traffic_secret>`
//! - TLS 1.3: `SERVER_TRAFFIC_SECRET_0 <hex> <hex_traffic_secret>`
//!
//! Also merges keys extracted from pcapng Decryption Secrets Blocks (DSB).

use crate::error::PcapError;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// TLS key material for a session.
#[derive(Debug, Clone)]
pub enum KeyMaterial {
    /// TLS 1.2 master secret (48 bytes).
    MasterSecret(Vec<u8>),
    /// TLS 1.3 client traffic secret for application data (32 or 48 bytes).
    ClientTrafficSecret0(Vec<u8>),
    /// TLS 1.3 server traffic secret for application data (32 or 48 bytes).
    ServerTrafficSecret0(Vec<u8>),
    /// TLS 1.3 client handshake traffic secret (32 or 48 bytes).
    ClientHandshakeTrafficSecret(Vec<u8>),
    /// TLS 1.3 server handshake traffic secret (32 or 48 bytes).
    ServerHandshakeTrafficSecret(Vec<u8>),
}

impl KeyMaterial {
    /// Returns the raw key bytes.
    #[must_use] 
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::MasterSecret(bytes) => bytes,
            Self::ClientTrafficSecret0(bytes) => bytes,
            Self::ServerTrafficSecret0(bytes) => bytes,
            Self::ClientHandshakeTrafficSecret(bytes) => bytes,
            Self::ServerHandshakeTrafficSecret(bytes) => bytes,
        }
    }

    /// Returns true if this is a TLS 1.2 master secret.
    #[must_use] 
    pub const fn is_tls12(&self) -> bool {
        matches!(self, Self::MasterSecret(_))
    }

    /// Returns true if this is a TLS 1.3 traffic secret.
    #[must_use] 
    pub const fn is_tls13(&self) -> bool {
        matches!(
            self,
            Self::ClientTrafficSecret0(_)
                | Self::ServerTrafficSecret0(_)
                | Self::ClientHandshakeTrafficSecret(_)
                | Self::ServerHandshakeTrafficSecret(_)
        )
    }
}

/// TLS key log storage.
///
/// Maps `client_random` (32 bytes) to a collection of key material.
/// For TLS 1.3, multiple secrets (client, server, handshake) may exist per `client_random`.
#[derive(Debug, Clone, Default)]
pub struct TlsKeyLog {
    keys: HashMap<Vec<u8>, Vec<KeyMaterial>>,
}

impl TlsKeyLog {
    /// Creates an empty key log.
    #[must_use] 
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads keys from an SSLKEYLOGFILE.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, PcapError> {
        let mut keylog = Self::new();
        keylog.load_file(path)?;
        Ok(keylog)
    }

    /// Loads keys from an SSLKEYLOGFILE and merges with existing keys.
    pub fn load_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), PcapError> {
        let file = File::open(path.as_ref())?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            self.parse_line(&line)?;
        }

        Ok(())
    }

    /// Parses a single line from an SSLKEYLOGFILE.
    ///
    /// Supported formats:
    /// - `CLIENT_RANDOM <hex_client_random> <hex_master_secret>`
    /// - `CLIENT_TRAFFIC_SECRET_0 <hex_client_random> <hex_traffic_secret>`
    /// - `SERVER_TRAFFIC_SECRET_0 <hex_client_random> <hex_traffic_secret>`
    /// - `CLIENT_HANDSHAKE_TRAFFIC_SECRET <hex_client_random> <hex_traffic_secret>`
    /// - `SERVER_HANDSHAKE_TRAFFIC_SECRET <hex_client_random> <hex_traffic_secret>`
    pub fn parse_line(&mut self, line: &str) -> Result<(), PcapError> {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            return Ok(());
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            return Ok(()); // Ignore malformed lines
        }

        let label = parts[0];
        let client_random = hex::decode(parts[1])
            .map_err(|e| PcapError::TlsKey(format!("invalid hex in client_random: {e}")))?;
        let key_material_hex = parts[2];
        let key_material_bytes = hex::decode(key_material_hex)
            .map_err(|e| PcapError::TlsKey(format!("invalid hex in key material: {e}")))?;

        // Validate client_random length (must be 32 bytes)
        if client_random.len() != 32 {
            return Err(PcapError::TlsKey(format!(
                "invalid client_random length: {} (expected 32)",
                client_random.len()
            )));
        }

        let key_material = match label {
            "CLIENT_RANDOM" => {
                // TLS 1.2 master secret (48 bytes)
                if key_material_bytes.len() != 48 {
                    return Err(PcapError::TlsKey(format!(
                        "invalid master_secret length: {} (expected 48)",
                        key_material_bytes.len()
                    )));
                }
                KeyMaterial::MasterSecret(key_material_bytes)
            }
            "CLIENT_TRAFFIC_SECRET_0" => {
                // TLS 1.3 client traffic secret (32 or 48 bytes)
                if key_material_bytes.len() != 32 && key_material_bytes.len() != 48 {
                    return Err(PcapError::TlsKey(format!(
                        "invalid traffic secret length: {} (expected 32 or 48)",
                        key_material_bytes.len()
                    )));
                }
                KeyMaterial::ClientTrafficSecret0(key_material_bytes)
            }
            "SERVER_TRAFFIC_SECRET_0" => {
                // TLS 1.3 server traffic secret (32 or 48 bytes)
                if key_material_bytes.len() != 32 && key_material_bytes.len() != 48 {
                    return Err(PcapError::TlsKey(format!(
                        "invalid traffic secret length: {} (expected 32 or 48)",
                        key_material_bytes.len()
                    )));
                }
                KeyMaterial::ServerTrafficSecret0(key_material_bytes)
            }
            "CLIENT_HANDSHAKE_TRAFFIC_SECRET" => {
                // TLS 1.3 client handshake traffic secret (32 or 48 bytes)
                if key_material_bytes.len() != 32 && key_material_bytes.len() != 48 {
                    return Err(PcapError::TlsKey(format!(
                        "invalid handshake secret length: {} (expected 32 or 48)",
                        key_material_bytes.len()
                    )));
                }
                KeyMaterial::ClientHandshakeTrafficSecret(key_material_bytes)
            }
            "SERVER_HANDSHAKE_TRAFFIC_SECRET" => {
                // TLS 1.3 server handshake traffic secret (32 or 48 bytes)
                if key_material_bytes.len() != 32 && key_material_bytes.len() != 48 {
                    return Err(PcapError::TlsKey(format!(
                        "invalid handshake secret length: {} (expected 32 or 48)",
                        key_material_bytes.len()
                    )));
                }
                KeyMaterial::ServerHandshakeTrafficSecret(key_material_bytes)
            }
            _ => {
                // Unknown label - ignore
                return Ok(());
            }
        };

        self.keys
            .entry(client_random)
            .or_default()
            .push(key_material);
        Ok(())
    }

    /// Inserts key material for a `client_random`.
    /// Multiple keys can exist for the same `client_random` (e.g., TLS 1.3 client + server secrets).
    pub fn insert(&mut self, client_random: Vec<u8>, key_material: KeyMaterial) {
        self.keys
            .entry(client_random)
            .or_default()
            .push(key_material);
    }

    /// Looks up all key material by `client_random`.
    #[must_use] 
    pub fn lookup(&self, client_random: &[u8]) -> Option<&[KeyMaterial]> {
        self.keys.get(client_random).map(std::vec::Vec::as_slice)
    }

    /// Looks up a specific key type by `client_random` and direction.
    ///
    /// For TLS 1.2, returns the master secret regardless of direction.
    /// For TLS 1.3, returns client or server traffic secret based on direction.
    #[must_use] 
    pub fn lookup_for_direction(
        &self,
        client_random: &[u8],
        direction: crate::tcp::StreamDirection,
    ) -> Option<&KeyMaterial> {
        let materials = self.lookup(client_random)?;

        // For TLS 1.2, return master secret
        if let Some(master) = materials.iter().find(|m| m.is_tls12()) {
            return Some(master);
        }

        // For TLS 1.3, select based on direction
        match direction {
            crate::tcp::StreamDirection::ClientToServer => materials
                .iter()
                .find(|m| matches!(m, KeyMaterial::ClientTrafficSecret0(_))),
            crate::tcp::StreamDirection::ServerToClient => materials
                .iter()
                .find(|m| matches!(m, KeyMaterial::ServerTrafficSecret0(_))),
        }
    }

    /// Merges DSB-extracted keys (from pcapng) with existing keys.
    ///
    /// DSB keys are in SSLKEYLOGFILE format and can be merged by parsing line-by-line.
    pub fn merge_dsb_keys(&mut self, dsb_data: &[u8]) -> Result<(), PcapError> {
        let dsb_str = std::str::from_utf8(dsb_data)
            .map_err(|e| PcapError::TlsKey(format!("invalid UTF-8 in DSB: {e}")))?;

        for line in dsb_str.lines() {
            self.parse_line(line)?;
        }

        Ok(())
    }

    /// Returns the number of stored keys.
    #[must_use] 
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns true if no keys are stored.
    #[must_use] 
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

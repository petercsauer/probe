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
    /// TLS 1.3 client traffic secret (32 or 48 bytes depending on cipher).
    ClientTrafficSecret0(Vec<u8>),
    /// TLS 1.3 server traffic secret (32 or 48 bytes depending on cipher).
    ServerTrafficSecret0(Vec<u8>),
}

impl KeyMaterial {
    /// Returns the raw key bytes.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            KeyMaterial::MasterSecret(bytes) => bytes,
            KeyMaterial::ClientTrafficSecret0(bytes) => bytes,
            KeyMaterial::ServerTrafficSecret0(bytes) => bytes,
        }
    }

    /// Returns true if this is a TLS 1.2 master secret.
    pub fn is_tls12(&self) -> bool {
        matches!(self, KeyMaterial::MasterSecret(_))
    }

    /// Returns true if this is a TLS 1.3 traffic secret.
    pub fn is_tls13(&self) -> bool {
        matches!(
            self,
            KeyMaterial::ClientTrafficSecret0(_) | KeyMaterial::ServerTrafficSecret0(_)
        )
    }
}

/// TLS key log storage.
///
/// Maps client_random (32 bytes) to key material.
#[derive(Debug, Clone, Default)]
pub struct TlsKeyLog {
    keys: HashMap<Vec<u8>, KeyMaterial>,
}

impl TlsKeyLog {
    /// Creates an empty key log.
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
            .map_err(|e| PcapError::TlsKey(format!("invalid hex in client_random: {}", e)))?;
        let key_material_hex = parts[2];
        let key_material_bytes = hex::decode(key_material_hex)
            .map_err(|e| PcapError::TlsKey(format!("invalid hex in key material: {}", e)))?;

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
            _ => {
                // Unknown label - ignore
                return Ok(());
            }
        };

        self.keys.insert(client_random, key_material);
        Ok(())
    }

    /// Inserts key material for a client_random.
    pub fn insert(&mut self, client_random: Vec<u8>, key_material: KeyMaterial) {
        self.keys.insert(client_random, key_material);
    }

    /// Looks up key material by client_random.
    pub fn lookup(&self, client_random: &[u8]) -> Option<&KeyMaterial> {
        self.keys.get(client_random)
    }

    /// Merges DSB-extracted keys (from pcapng) with existing keys.
    ///
    /// DSB keys are in SSLKEYLOGFILE format and can be merged by parsing line-by-line.
    pub fn merge_dsb_keys(&mut self, dsb_data: &[u8]) -> Result<(), PcapError> {
        let dsb_str = std::str::from_utf8(dsb_data)
            .map_err(|e| PcapError::TlsKey(format!("invalid UTF-8 in DSB: {}", e)))?;

        for line in dsb_str.lines() {
            self.parse_line(line)?;
        }

        Ok(())
    }

    /// Returns the number of stored keys.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns true if no keys are stored.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

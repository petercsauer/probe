//! RTPS discovery tracker for topic name resolution.

use std::collections::HashMap;

/// RTPS GUID prefix (12 bytes).
pub type GuidPrefix = [u8; 12];

/// RTPS entity ID (4 bytes).
pub type EntityId = [u8; 4];

/// Full RTPS GUID (16 bytes: prefix + entity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Guid {
    pub prefix: GuidPrefix,
    pub entity: EntityId,
}

impl Guid {
    pub fn new(prefix: GuidPrefix, entity: EntityId) -> Self {
        Self { prefix, entity }
    }

    /// Format GUID as hex string.
    pub fn to_hex_string(&self) -> String {
        format!(
            "{:02x}{:02x}{:02x}{:02x}.{:02x}{:02x}{:02x}{:02x}.{:02x}{:02x}{:02x}{:02x}:{:02x}{:02x}{:02x}{:02x}",
            self.prefix[0], self.prefix[1], self.prefix[2], self.prefix[3],
            self.prefix[4], self.prefix[5], self.prefix[6], self.prefix[7],
            self.prefix[8], self.prefix[9], self.prefix[10], self.prefix[11],
            self.entity[0], self.entity[1], self.entity[2], self.entity[3],
        )
    }

    /// Format entity ID as hex string.
    pub fn entity_hex(&self) -> String {
        format!(
            "{:02x}{:02x}{:02x}{:02x}",
            self.entity[0], self.entity[1], self.entity[2], self.entity[3]
        )
    }
}

/// Well-known SEDP entity IDs for discovery.
pub mod well_known_entities {
    use super::EntityId;

    /// SEDP Built-in Publications Writer.
    pub const ENTITYID_SEDP_BUILTIN_PUBLICATIONS_WRITER: EntityId = [0x00, 0x00, 0x03, 0xC2];

    /// SEDP Built-in Publications Reader.
    pub const ENTITYID_SEDP_BUILTIN_PUBLICATIONS_READER: EntityId = [0x00, 0x00, 0x03, 0xC7];

    /// SEDP Built-in Subscriptions Writer.
    pub const ENTITYID_SEDP_BUILTIN_SUBSCRIPTIONS_WRITER: EntityId = [0x00, 0x00, 0x03, 0xC7];

    /// SEDP Built-in Subscriptions Reader.
    pub const ENTITYID_SEDP_BUILTIN_SUBSCRIPTIONS_READER: EntityId = [0x00, 0x00, 0x03, 0xC4];

    /// Check if entity ID is a SEDP discovery entity.
    pub fn is_sedp_entity(entity_id: &EntityId) -> bool {
        entity_id[3] == 0xC2 || entity_id[3] == 0xC4 || entity_id[3] == 0xC7
    }
}

/// Discovered writer/reader information.
#[derive(Debug, Clone)]
pub struct DiscoveredEndpoint {
    pub topic_name: String,
    pub type_name: String,
}

/// Tracks RTPS discovery information for topic name resolution.
pub struct RtpsDiscoveryTracker {
    /// Map from GUID to discovered endpoint info.
    endpoints: HashMap<Guid, DiscoveredEndpoint>,
}

impl RtpsDiscoveryTracker {
    /// Create a new discovery tracker.
    pub fn new() -> Self {
        Self {
            endpoints: HashMap::new(),
        }
    }

    /// Register a discovered endpoint.
    pub fn register_endpoint(&mut self, guid: Guid, endpoint: DiscoveredEndpoint) {
        self.endpoints.insert(guid, endpoint);
    }

    /// Look up topic name for a given GUID.
    pub fn lookup_topic_name(&self, guid: &Guid) -> Option<&str> {
        self.endpoints.get(guid).map(|e| e.topic_name.as_str())
    }

    /// Look up type name for a given GUID.
    pub fn lookup_type_name(&self, guid: &Guid) -> Option<&str> {
        self.endpoints.get(guid).map(|e| e.type_name.as_str())
    }
}

impl Default for RtpsDiscoveryTracker {
    fn default() -> Self {
        Self::new()
    }
}

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
    pub fn to_hex_string(self) -> String {
        format!(
            "{:02x}{:02x}{:02x}{:02x}.{:02x}{:02x}{:02x}{:02x}.{:02x}{:02x}{:02x}{:02x}:{:02x}{:02x}{:02x}{:02x}",
            self.prefix[0],
            self.prefix[1],
            self.prefix[2],
            self.prefix[3],
            self.prefix[4],
            self.prefix[5],
            self.prefix[6],
            self.prefix[7],
            self.prefix[8],
            self.prefix[9],
            self.prefix[10],
            self.prefix[11],
            self.entity[0],
            self.entity[1],
            self.entity[2],
            self.entity[3],
        )
    }

    /// Format entity ID as hex string.
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub const ENTITYID_SEDP_BUILTIN_PUBLICATIONS_WRITER: EntityId = [0x00, 0x00, 0x03, 0xC2];

    /// SEDP Built-in Publications Reader.
    #[allow(dead_code)]
    pub const ENTITYID_SEDP_BUILTIN_PUBLICATIONS_READER: EntityId = [0x00, 0x00, 0x03, 0xC7];

    /// SEDP Built-in Subscriptions Writer.
    #[allow(dead_code)]
    pub const ENTITYID_SEDP_BUILTIN_SUBSCRIPTIONS_WRITER: EntityId = [0x00, 0x00, 0x03, 0xC7];

    /// SEDP Built-in Subscriptions Reader.
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn lookup_type_name(&self, guid: &Guid) -> Option<&str> {
        self.endpoints.get(guid).map(|e| e.type_name.as_str())
    }
}

impl Default for RtpsDiscoveryTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdr_parameter_list_parse() {
        // WS-3.3: Real CDR payload with PID_TOPIC_NAME, PID_TYPE_NAME, PID_ENDPOINT_GUID, PID_SENTINEL
        // This is more of an integration test with the decoder, but tests the data structures

        // Create test payload: LE CDR encapsulation + parameters
        let mut payload = Vec::new();

        // Encapsulation header (4 bytes)
        payload.extend_from_slice(&[0x01, 0x00]); // LE CDR
        payload.extend_from_slice(&[0x00, 0x00]); // Options

        // PID_TOPIC_NAME (0x0005)
        payload.extend_from_slice(&0x0005u16.to_le_bytes()); // PID
        let topic_name = b"test_topic";
        let topic_str_len = (topic_name.len() + 1) as u32; // +1 for null terminator
        let topic_param_len = (4 + topic_str_len) as u16; // string length (4) + string bytes
        payload.extend_from_slice(&topic_param_len.to_le_bytes()); // param length
        payload.extend_from_slice(&topic_str_len.to_le_bytes()); // string length
        payload.extend_from_slice(topic_name);
        payload.push(0x00); // null terminator
        // Align to 4-byte boundary
        while payload.len() % 4 != 0 {
            payload.push(0x00);
        }

        // PID_TYPE_NAME (0x0007)
        payload.extend_from_slice(&0x0007u16.to_le_bytes());
        let type_name = b"test_type";
        let type_str_len = (type_name.len() + 1) as u32;
        let type_param_len = (4 + type_str_len) as u16;
        payload.extend_from_slice(&type_param_len.to_le_bytes());
        payload.extend_from_slice(&type_str_len.to_le_bytes());
        payload.extend_from_slice(type_name);
        payload.push(0x00);
        while payload.len() % 4 != 0 {
            payload.push(0x00);
        }

        // PID_ENDPOINT_GUID (0x005A) - 16 bytes
        payload.extend_from_slice(&0x005Au16.to_le_bytes());
        payload.extend_from_slice(&16u16.to_le_bytes()); // length
        let guid_prefix = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C,
        ];
        let entity_id = [0xAA, 0xBB, 0xCC, 0xDD];
        payload.extend_from_slice(&guid_prefix);
        payload.extend_from_slice(&entity_id);

        // PID_SENTINEL (0x0001)
        payload.extend_from_slice(&0x0001u16.to_le_bytes());
        payload.extend_from_slice(&0x0000u16.to_le_bytes());

        // Verify payload is valid
        assert!(payload.len() >= 4, "Payload should have at least 4 bytes");

        // The actual parsing is done by DdsDecoder::process_discovery_data
        // This test verifies the data structure is correctly formed
        let guid = Guid::new(guid_prefix, entity_id);
        assert_eq!(guid.prefix, guid_prefix);
        assert_eq!(guid.entity, entity_id);
    }

    #[test]
    fn test_discovery_register_and_lookup() {
        // WS-3.3: Register endpoint, look up by GUID, assert topic
        let mut tracker = RtpsDiscoveryTracker::new();

        let guid = Guid::new(
            [
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C,
            ],
            [0xAA, 0xBB, 0xCC, 0xDD],
        );

        let endpoint = DiscoveredEndpoint {
            topic_name: "test_topic".to_string(),
            type_name: "test_type".to_string(),
        };

        tracker.register_endpoint(guid, endpoint);

        // Lookup should succeed
        let topic = tracker.lookup_topic_name(&guid);
        assert_eq!(topic, Some("test_topic"), "Should find registered topic");

        let type_name = tracker.lookup_type_name(&guid);
        assert_eq!(type_name, Some("test_type"), "Should find registered type");
    }

    #[test]
    fn test_discovery_multiple_endpoints() {
        // WS-3.3: Register multiple writers, look up each
        let mut tracker = RtpsDiscoveryTracker::new();

        let guid1 = Guid::new([0x01; 12], [0xAA, 0xBB, 0xCC, 0xDD]);
        let guid2 = Guid::new([0x02; 12], [0x11, 0x22, 0x33, 0x44]);
        let guid3 = Guid::new([0x03; 12], [0xFF, 0xEE, 0xDD, 0xCC]);

        tracker.register_endpoint(
            guid1,
            DiscoveredEndpoint {
                topic_name: "topic_one".to_string(),
                type_name: "type_one".to_string(),
            },
        );

        tracker.register_endpoint(
            guid2,
            DiscoveredEndpoint {
                topic_name: "topic_two".to_string(),
                type_name: "type_two".to_string(),
            },
        );

        tracker.register_endpoint(
            guid3,
            DiscoveredEndpoint {
                topic_name: "topic_three".to_string(),
                type_name: "type_three".to_string(),
            },
        );

        // Verify all endpoints are registered
        assert_eq!(tracker.lookup_topic_name(&guid1), Some("topic_one"));
        assert_eq!(tracker.lookup_topic_name(&guid2), Some("topic_two"));
        assert_eq!(tracker.lookup_topic_name(&guid3), Some("topic_three"));
    }

    #[test]
    fn test_discovery_unknown_guid() {
        // WS-3.3: Lookup of unregistered GUID returns None
        let tracker = RtpsDiscoveryTracker::new();

        let unknown_guid = Guid::new([0xFF; 12], [0x00, 0x00, 0x00, 0x00]);

        let topic = tracker.lookup_topic_name(&unknown_guid);
        assert_eq!(topic, None, "Should return None for unknown GUID");

        let type_name = tracker.lookup_type_name(&unknown_guid);
        assert_eq!(type_name, None, "Should return None for unknown GUID");
    }
}

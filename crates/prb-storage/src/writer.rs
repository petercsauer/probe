//! MCAP session writer.

use crate::error::Result;
use crate::metadata::SessionMetadata;
use mcap::Writer;
use mcap::records::MessageHeader;
use prb_core::DebugEvent;
use prb_schema::SchemaRegistry;
use std::collections::BTreeMap;
use std::io::{Seek, Write};

/// Channel key for routing events to channels.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct ChannelKey {
    adapter: String,
    origin: String,
}

/// MCAP session writer for `DebugEvents`.
pub struct SessionWriter<W: Write + Seek> {
    writer: Writer<W>,
    metadata: SessionMetadata,
    channels: BTreeMap<ChannelKey, u16>,
    channel_sequences: BTreeMap<u16, u32>,
}

impl<W: Write + Seek> SessionWriter<W> {
    /// Create a new `SessionWriter`.
    pub fn new(writer: W, metadata: SessionMetadata) -> Result<Self> {
        let writer = Writer::new(writer)?;
        Ok(Self {
            writer,
            metadata,
            channels: BTreeMap::new(),
            channel_sequences: BTreeMap::new(),
        })
    }

    /// Write a `DebugEvent` to the session.
    pub fn write_event(&mut self, event: &DebugEvent) -> Result<()> {
        // Determine channel key
        let channel_key = ChannelKey {
            adapter: event.source.adapter.clone(),
            origin: event.source.origin.clone(),
        };

        // Get or create channel
        let channel_id = if let Some(&id) = self.channels.get(&channel_key) {
            id
        } else {
            let topic = format!("events/{}/{}", channel_key.adapter, channel_key.origin);

            // Create a schema for DebugEvent
            let schema_id =
                self.writer
                    .add_schema("prb.DebugEvent", "jsonschema", br#"{"type":"object"}"#)?;

            // Create channel
            let channel_id =
                self.writer
                    .add_channel(schema_id, &topic, "json", &BTreeMap::new())?;

            self.channels.insert(channel_key, channel_id);
            self.channel_sequences.insert(channel_id, 0);
            channel_id
        };

        // Get and increment sequence number
        let sequence = self
            .channel_sequences
            .get_mut(&channel_id)
            .expect("channel_id must exist in channel_sequences");
        let seq = *sequence;
        *sequence += 1;

        // Serialize event to JSON
        let data = serde_json::to_vec(event)?;

        // Write message
        let header = MessageHeader {
            channel_id,
            sequence: seq,
            log_time: event.timestamp.as_nanos(),
            publish_time: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64,
        };

        self.writer.write_to_known_channel(&header, &data)?;

        Ok(())
    }

    /// Embed schemas from a registry into the session.
    ///
    /// This should be called before `finish()` to ensure schemas are available when
    /// the session is read.
    pub fn embed_schemas(&mut self, registry: &SchemaRegistry) -> Result<()> {
        // Get all descriptor sets from the registry
        for (idx, desc_bytes) in registry.descriptor_sets().iter().enumerate() {
            let schema_name = format!("protobuf_descriptors_{idx}");
            self.writer
                .add_schema(&schema_name, "protobuf", desc_bytes)?;
        }
        Ok(())
    }

    /// Finalize the MCAP file and write metadata.
    pub fn finish(mut self) -> Result<()> {
        // Write session metadata
        let metadata_json = serde_json::to_string(&self.metadata)?;
        let metadata_map: BTreeMap<String, String> = serde_json::from_str(&metadata_json)?;

        let metadata = mcap::records::Metadata {
            name: "session_info".to_string(),
            metadata: metadata_map,
        };
        self.writer.write_metadata(&metadata)?;

        // Finalize the MCAP file
        self.writer.finish()?;

        Ok(())
    }
}

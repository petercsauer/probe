//! MCAP session reader.

use crate::error::{Result, StorageError};
use crate::metadata::SessionMetadata;
use mcap::MessageStream;
use prb_core::DebugEvent;
use prb_schema::SchemaRegistry;
use std::fs::File;
use std::path::Path;

/// Channel information from the MCAP file.
#[derive(Debug, Clone)]
pub struct ChannelInfo {
    /// Channel ID.
    pub id: u16,
    /// Channel topic.
    pub topic: String,
    /// Message count (populated during iteration).
    pub message_count: u64,
}

/// MCAP session reader for `DebugEvents`.
pub struct SessionReader {
    mapped: memmap2::Mmap,
}

impl SessionReader {
    /// Open an MCAP file for reading.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path.as_ref())?;
        let mapped = unsafe { memmap2::Mmap::map(&file)? };

        // Validate that it's a valid MCAP file
        MessageStream::new(&mapped).map_err(|e| {
            StorageError::InvalidSession(format!("Failed to parse MCAP file: {e}"))
        })?;

        Ok(Self { mapped })
    }

    /// Iterate over all events in the session.
    pub fn events(&self) -> impl Iterator<Item = Result<DebugEvent>> + '_ {
        // MessageStream::new returns Result<MessageStream, Error>
        match MessageStream::new(&self.mapped) {
            Ok(stream) => itertools::Either::Left(stream.map(|msg_result| {
                msg_result.map_err(StorageError::Mcap).and_then(|msg| {
                    serde_json::from_slice::<DebugEvent>(&msg.data).map_err(StorageError::Json)
                })
            })),
            Err(e) => itertools::Either::Right(std::iter::once(Err(StorageError::Mcap(e)))),
        }
    }

    /// Read session metadata from the MCAP file.
    pub fn metadata(&self) -> Result<Option<SessionMetadata>> {
        // Use LinearReader to scan for Metadata records
        use mcap::read::LinearReader;
        use mcap::records::Record;

        let reader = LinearReader::new(&self.mapped)?;
        for record in reader {
            let record = record?;
            if let Record::Metadata(metadata) = record
                && metadata.name == "session_info"
            {
                // Convert BTreeMap to JSON and deserialize
                let metadata_json = serde_json::to_string(&metadata.metadata)?;
                let session_metadata: SessionMetadata = serde_json::from_str(&metadata_json)?;
                return Ok(Some(session_metadata));
            }
        }

        Ok(None)
    }

    /// Get channel information from the MCAP file.
    pub fn channels(&self) -> Result<Vec<ChannelInfo>> {
        let summary = match mcap::read::Summary::read(&self.mapped)? {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };

        let mut channels = Vec::new();

        for (channel_id, channel) in &summary.channels {
            // Count messages for this channel
            let message_count = summary
                .stats
                .as_ref()
                .and_then(|stats| stats.channel_message_counts.get(channel_id))
                .copied()
                .unwrap_or(0);

            channels.push(ChannelInfo {
                id: *channel_id,
                topic: channel.topic.clone(),
                message_count,
            });
        }

        Ok(channels)
    }

    /// Extract embedded schemas from the MCAP file.
    ///
    /// Returns a `SchemaRegistry` populated with all protobuf schemas found in the session.
    pub fn extract_schemas(&self) -> Result<SchemaRegistry> {
        use mcap::read::LinearReader;
        use mcap::records::Record;

        let mut registry = SchemaRegistry::new();

        let reader = LinearReader::new(&self.mapped)?;
        for record in reader {
            let record = record?;
            if let Record::Schema { header, data } = record {
                // Only load protobuf schemas
                if header.encoding == "protobuf" {
                    registry.load_descriptor_set(&data).map_err(|e| {
                        StorageError::InvalidSession(format!(
                            "Failed to load embedded schema: {e}"
                        ))
                    })?;
                }
            }
        }

        Ok(registry)
    }
}

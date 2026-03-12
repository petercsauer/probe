//! JSON fixture adapter implementation.

use crate::error::FixtureError;
use crate::format::{FixtureEvent, FixtureFile};
use bytes::Bytes;
use camino::Utf8PathBuf;
use prb_core::{
    CaptureAdapter, CoreError, DebugEvent, Direction, EventSource, NetworkAddr, Payload, Timestamp,
    TransportKind,
};
use std::fs;

/// `CaptureAdapter` that reads JSON fixture files.
pub struct JsonFixtureAdapter {
    path: Utf8PathBuf,
    events: Option<Vec<FixtureEvent>>,
}

impl JsonFixtureAdapter {
    /// Create a new JSON fixture adapter from a file path.
    #[must_use] 
    pub const fn new(path: Utf8PathBuf) -> Self {
        Self { path, events: None }
    }

    /// Load and parse the fixture file.
    fn load(&mut self) -> Result<(), FixtureError> {
        if self.events.is_some() {
            return Ok(());
        }

        let content = fs::read_to_string(&self.path)?;
        let fixture: FixtureFile = serde_json::from_str(&content)?;

        // Validate version
        if fixture.version != 1 {
            return Err(FixtureError::UnsupportedVersion(fixture.version));
        }

        self.events = Some(fixture.events);
        Ok(())
    }

    /// Convert a `FixtureEvent` to a `DebugEvent`.
    fn convert_event(&self, event: FixtureEvent) -> Result<DebugEvent, FixtureError> {
        // Parse transport
        let transport = parse_transport(&event.transport)?;

        // Parse direction
        let direction = parse_direction(&event.direction)?;

        // Validate and decode payload
        let payload = match (event.payload_base64, event.payload_utf8) {
            (Some(b64), None) => {
                let decoded = base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    b64.as_bytes(),
                )
                .map_err(|e| FixtureError::Base64Decode(e.to_string()))?;
                Payload::Raw {
                    raw: Bytes::from(decoded),
                }
            }
            (None, Some(utf8)) => Payload::Raw {
                raw: Bytes::from(utf8.into_bytes()),
            },
            (Some(_), Some(_)) => {
                return Err(FixtureError::invalid_format(
                    "cannot specify both payload_base64 and payload_utf8",
                ));
            }
            (None, None) => {
                return Err(FixtureError::invalid_format(
                    "must specify either payload_base64 or payload_utf8",
                ));
            }
        };

        // Build event source
        let network = event.source.and_then(|src| {
            if let (Some(src_addr), Some(dst_addr)) = (src.src, src.dst) {
                Some(NetworkAddr {
                    src: src_addr,
                    dst: dst_addr,
                })
            } else {
                None
            }
        });

        let source = EventSource {
            adapter: "json-fixture".to_string(),
            origin: self.path.to_string(),
            network,
        };

        // Build the event
        let debug_event = DebugEvent::builder()
            .timestamp(Timestamp::from_nanos(event.timestamp_ns))
            .source(source)
            .transport(transport)
            .direction(direction)
            .payload(payload)
            .build();

        // Add metadata
        let mut debug_event = debug_event;
        for (key, value) in event.metadata {
            debug_event.metadata.insert(key, value);
        }

        Ok(debug_event)
    }

    /// Map `FixtureError` to `CoreError` for trait compliance.
    fn map_error(e: FixtureError) -> CoreError {
        match e {
            FixtureError::Base64Decode(msg) => {
                CoreError::PayloadDecode(format!("base64 decode: {msg}"))
            }
            FixtureError::InvalidTransport(t) => CoreError::UnsupportedTransport(t),
            FixtureError::InvalidFormat(msg) => {
                CoreError::PayloadDecode(format!("invalid format: {msg}"))
            }
            FixtureError::Parse { source } => CoreError::from(source),
            FixtureError::Io { source } => {
                CoreError::PayloadDecode(format!("I/O error: {source}"))
            }
            FixtureError::UnsupportedVersion(v) => {
                CoreError::PayloadDecode(format!("unsupported version: {v}"))
            }
            FixtureError::InvalidDirection(d) => {
                CoreError::PayloadDecode(format!("invalid direction: {d}"))
            }
            FixtureError::Core(e) => e,
        }
    }
}

impl CaptureAdapter for JsonFixtureAdapter {
    fn name(&self) -> &'static str {
        "json-fixture"
    }

    fn ingest(&mut self) -> Box<dyn Iterator<Item = Result<DebugEvent, CoreError>> + '_> {
        // Attempt to load the file
        if let Err(e) = self.load() {
            return Box::new(std::iter::once(Err(Self::map_error(e))));
        }

        // Take ownership of events to iterate
        let events = self.events.take().unwrap_or_default();

        Box::new(
            events
                .into_iter()
                .map(|event| self.convert_event(event).map_err(Self::map_error)),
        )
    }
}

/// Parse a transport string into `TransportKind`.
fn parse_transport(s: &str) -> Result<TransportKind, FixtureError> {
    match s.to_lowercase().as_str() {
        "grpc" => Ok(TransportKind::Grpc),
        "zmq" => Ok(TransportKind::Zmq),
        "dds-rtps" => Ok(TransportKind::DdsRtps),
        "raw-tcp" | "tcp" => Ok(TransportKind::RawTcp),
        "raw-udp" | "udp" => Ok(TransportKind::RawUdp),
        "json-fixture" => Ok(TransportKind::JsonFixture),
        _ => Err(FixtureError::InvalidTransport(s.to_string())),
    }
}

/// Parse a direction string into Direction.
fn parse_direction(s: &str) -> Result<Direction, FixtureError> {
    match s.to_lowercase().as_str() {
        "inbound" => Ok(Direction::Inbound),
        "outbound" => Ok(Direction::Outbound),
        "unknown" => Ok(Direction::Unknown),
        _ => Err(FixtureError::InvalidDirection(s.to_string())),
    }
}

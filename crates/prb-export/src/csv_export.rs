use crate::{ExportError, Exporter};
use prb_core::{DebugEvent, Payload, Timestamp};
use serde::Serialize;
use std::io::Write;

pub struct CsvExporter;

#[derive(Serialize)]
struct EventRow {
    id: u64,
    timestamp_nanos: u64,
    timestamp_iso: String,
    adapter: String,
    origin: String,
    src_addr: String,
    dst_addr: String,
    transport: String,
    direction: String,
    payload_type: String,
    payload_size: usize,
    schema_name: String,
    decoded_fields: String,
    metadata: String,
    grpc_method: String,
    grpc_status: String,
    zmq_topic: String,
    dds_topic_name: String,
    sequence: String,
    warnings: String,
}

fn timestamp_to_iso(ts: Timestamp) -> String {
    let nanos = ts.as_nanos();
    let secs = (nanos / 1_000_000_000) as i64;
    let subsec_nanos = (nanos % 1_000_000_000) as u32;
    chrono::DateTime::from_timestamp(secs, subsec_nanos).map_or_else(|| format!("{nanos}ns"), |dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

impl EventRow {
    fn from_event(event: &DebugEvent) -> Self {
        let (payload_type, payload_size, schema_name, decoded_fields) = match &event.payload {
            Payload::Raw { raw } => ("raw".to_string(), raw.len(), String::new(), String::new()),
            Payload::Decoded {
                raw,
                fields,
                schema_name,
            } => (
                "decoded".to_string(),
                raw.len(),
                schema_name.clone().unwrap_or_default(),
                serde_json::to_string(fields).unwrap_or_default(),
            ),
        };

        let metadata_json = if event.metadata.is_empty() {
            String::new()
        } else {
            serde_json::to_string(&event.metadata).unwrap_or_default()
        };

        let warnings_json = if event.warnings.is_empty() {
            String::new()
        } else {
            serde_json::to_string(&event.warnings).unwrap_or_default()
        };

        let (src_addr, dst_addr) = event
            .source
            .network
            .as_ref()
            .map(|n| (n.src.clone(), n.dst.clone()))
            .unwrap_or_default();

        Self {
            id: event.id.as_u64(),
            timestamp_nanos: event.timestamp.as_nanos(),
            timestamp_iso: timestamp_to_iso(event.timestamp),
            adapter: event.source.adapter.clone(),
            origin: event.source.origin.clone(),
            src_addr,
            dst_addr,
            transport: event.transport.to_string(),
            direction: event.direction.to_string(),
            payload_type,
            payload_size,
            schema_name,
            decoded_fields,
            metadata: metadata_json,
            grpc_method: event
                .metadata
                .get("grpc.method")
                .cloned()
                .unwrap_or_default(),
            grpc_status: event
                .metadata
                .get("grpc.status")
                .cloned()
                .unwrap_or_default(),
            zmq_topic: event.metadata.get("zmq.topic").cloned().unwrap_or_default(),
            dds_topic_name: event
                .metadata
                .get("dds.topic_name")
                .cloned()
                .unwrap_or_default(),
            sequence: event.sequence.map(|s| s.to_string()).unwrap_or_default(),
            warnings: warnings_json,
        }
    }
}

impl Exporter for CsvExporter {
    fn format_name(&self) -> &'static str {
        "csv"
    }

    fn file_extension(&self) -> &'static str {
        "csv"
    }

    fn export(&self, events: &[DebugEvent], writer: &mut dyn Write) -> Result<(), ExportError> {
        let mut wtr = csv::Writer::from_writer(writer);

        // Write headers even for empty events
        if events.is_empty() {
            wtr.write_record([
                "id",
                "timestamp_nanos",
                "timestamp_iso",
                "adapter",
                "origin",
                "src_addr",
                "dst_addr",
                "transport",
                "direction",
                "payload_type",
                "payload_size",
                "schema_name",
                "decoded_fields",
                "metadata",
                "grpc_method",
                "grpc_status",
                "zmq_topic",
                "dds_topic_name",
                "sequence",
                "warnings",
            ])?;
        }

        for event in events {
            wtr.serialize(EventRow::from_event(event))?;
        }
        wtr.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use prb_core::*;

    fn sample_event() -> DebugEvent {
        DebugEvent::builder()
            .id(EventId::from_raw(1))
            .timestamp(Timestamp::from_nanos(1_710_000_000_000_000_000))
            .source(EventSource {
                adapter: "pcap".into(),
                origin: "test.pcap".into(),
                network: Some(NetworkAddr {
                    src: "10.0.0.1:50051".into(),
                    dst: "10.0.0.2:8080".into(),
                }),
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"hello"),
            })
            .metadata("grpc.method", "/api.v1.Users/Get")
            .build()
    }

    #[test]
    fn csv_round_trip() {
        let events = vec![sample_event()];
        let mut buf = Vec::new();
        CsvExporter.export(&events, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("id,"));
        assert!(output.contains("/api.v1.Users/Get"));
        assert!(output.contains("10.0.0.1:50051"));
        assert!(output.contains("gRPC"));

        let mut rdr = csv::Reader::from_reader(output.as_bytes());
        let headers = rdr.headers().unwrap();
        assert_eq!(headers.get(0), Some("id"));
        assert_eq!(headers.get(7), Some("transport"));

        let records: Vec<_> = rdr.records().collect();
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn csv_empty_events() {
        let mut buf = Vec::new();
        CsvExporter.export(&[], &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("id,"));
        let mut rdr = csv::Reader::from_reader(output.as_bytes());
        let records: Vec<_> = rdr.records().collect();
        assert_eq!(records.len(), 0);
    }
}

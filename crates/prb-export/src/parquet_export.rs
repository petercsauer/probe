use crate::{ExportError, Exporter};
use arrow::array::{ArrayRef, StringBuilder, UInt64Builder};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use prb_core::{DebugEvent, Payload, Timestamp};
use std::io::Write;
use std::sync::Arc;

pub struct ParquetExporter;

fn timestamp_to_iso(ts: Timestamp) -> String {
    let nanos = ts.as_nanos();
    let secs = (nanos / 1_000_000_000) as i64;
    let subsec_nanos = (nanos % 1_000_000_000) as u32;
    chrono::DateTime::from_timestamp(secs, subsec_nanos)
        .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_else(|| format!("{}ns", nanos))
}

fn create_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::UInt64, false),
        Field::new("timestamp_nanos", DataType::UInt64, false),
        Field::new("timestamp_iso", DataType::Utf8, false),
        Field::new("adapter", DataType::Utf8, false),
        Field::new("origin", DataType::Utf8, false),
        Field::new("src_addr", DataType::Utf8, true),
        Field::new("dst_addr", DataType::Utf8, true),
        Field::new("transport", DataType::Utf8, false),
        Field::new("direction", DataType::Utf8, false),
        Field::new("payload_type", DataType::Utf8, false),
        Field::new("payload_size", DataType::UInt64, false),
        Field::new("schema_name", DataType::Utf8, true),
        Field::new("decoded_fields_json", DataType::Utf8, true),
        Field::new("metadata_json", DataType::Utf8, false),
        Field::new("grpc_method", DataType::Utf8, true),
        Field::new("grpc_status", DataType::Utf8, true),
        Field::new("zmq_topic", DataType::Utf8, true),
        Field::new("dds_topic_name", DataType::Utf8, true),
        Field::new("sequence", DataType::UInt64, true),
        Field::new("warnings_json", DataType::Utf8, true),
    ]))
}

fn events_to_record_batch(events: &[DebugEvent]) -> Result<RecordBatch, ExportError> {
    let schema = create_schema();

    let mut id_builder = UInt64Builder::new();
    let mut timestamp_nanos_builder = UInt64Builder::new();
    let mut timestamp_iso_builder = StringBuilder::new();
    let mut adapter_builder = StringBuilder::new();
    let mut origin_builder = StringBuilder::new();
    let mut src_addr_builder = StringBuilder::new();
    let mut dst_addr_builder = StringBuilder::new();
    let mut transport_builder = StringBuilder::new();
    let mut direction_builder = StringBuilder::new();
    let mut payload_type_builder = StringBuilder::new();
    let mut payload_size_builder = UInt64Builder::new();
    let mut schema_name_builder = StringBuilder::new();
    let mut decoded_fields_builder = StringBuilder::new();
    let mut metadata_json_builder = StringBuilder::new();
    let mut grpc_method_builder = StringBuilder::new();
    let mut grpc_status_builder = StringBuilder::new();
    let mut zmq_topic_builder = StringBuilder::new();
    let mut dds_topic_name_builder = StringBuilder::new();
    let mut sequence_builder = UInt64Builder::new();
    let mut warnings_json_builder = StringBuilder::new();

    for event in events {
        id_builder.append_value(event.id.as_u64());
        timestamp_nanos_builder.append_value(event.timestamp.as_nanos());
        timestamp_iso_builder.append_value(timestamp_to_iso(event.timestamp));
        adapter_builder.append_value(&event.source.adapter);
        origin_builder.append_value(&event.source.origin);

        if let Some(ref net) = event.source.network {
            src_addr_builder.append_value(&net.src);
            dst_addr_builder.append_value(&net.dst);
        } else {
            src_addr_builder.append_null();
            dst_addr_builder.append_null();
        }

        transport_builder.append_value(event.transport.to_string());
        direction_builder.append_value(event.direction.to_string());

        match &event.payload {
            Payload::Raw { raw } => {
                payload_type_builder.append_value("raw");
                payload_size_builder.append_value(raw.len() as u64);
                schema_name_builder.append_null();
                decoded_fields_builder.append_null();
            }
            Payload::Decoded {
                raw,
                fields,
                schema_name,
            } => {
                payload_type_builder.append_value("decoded");
                payload_size_builder.append_value(raw.len() as u64);
                if let Some(name) = schema_name {
                    schema_name_builder.append_value(name);
                } else {
                    schema_name_builder.append_null();
                }
                if let Ok(json) = serde_json::to_string(fields) {
                    decoded_fields_builder.append_value(json);
                } else {
                    decoded_fields_builder.append_null();
                }
            }
        }

        if let Ok(meta_json) = serde_json::to_string(&event.metadata) {
            metadata_json_builder.append_value(meta_json);
        } else {
            metadata_json_builder.append_value("{}");
        }

        if let Some(method) = event.metadata.get("grpc.method") {
            grpc_method_builder.append_value(method);
        } else {
            grpc_method_builder.append_null();
        }

        if let Some(status) = event.metadata.get("grpc.status") {
            grpc_status_builder.append_value(status);
        } else {
            grpc_status_builder.append_null();
        }

        if let Some(topic) = event.metadata.get("zmq.topic") {
            zmq_topic_builder.append_value(topic);
        } else {
            zmq_topic_builder.append_null();
        }

        if let Some(topic) = event.metadata.get("dds.topic_name") {
            dds_topic_name_builder.append_value(topic);
        } else {
            dds_topic_name_builder.append_null();
        }

        if let Some(seq) = event.sequence {
            sequence_builder.append_value(seq);
        } else {
            sequence_builder.append_null();
        }

        if !event.warnings.is_empty() {
            if let Ok(warnings_json) = serde_json::to_string(&event.warnings) {
                warnings_json_builder.append_value(warnings_json);
            } else {
                warnings_json_builder.append_null();
            }
        } else {
            warnings_json_builder.append_null();
        }
    }

    let columns: Vec<ArrayRef> = vec![
        Arc::new(id_builder.finish()),
        Arc::new(timestamp_nanos_builder.finish()),
        Arc::new(timestamp_iso_builder.finish()),
        Arc::new(adapter_builder.finish()),
        Arc::new(origin_builder.finish()),
        Arc::new(src_addr_builder.finish()),
        Arc::new(dst_addr_builder.finish()),
        Arc::new(transport_builder.finish()),
        Arc::new(direction_builder.finish()),
        Arc::new(payload_type_builder.finish()),
        Arc::new(payload_size_builder.finish()),
        Arc::new(schema_name_builder.finish()),
        Arc::new(decoded_fields_builder.finish()),
        Arc::new(metadata_json_builder.finish()),
        Arc::new(grpc_method_builder.finish()),
        Arc::new(grpc_status_builder.finish()),
        Arc::new(zmq_topic_builder.finish()),
        Arc::new(dds_topic_name_builder.finish()),
        Arc::new(sequence_builder.finish()),
        Arc::new(warnings_json_builder.finish()),
    ];

    RecordBatch::try_new(schema, columns)
        .map_err(|e| ExportError::Other(format!("Arrow error: {}", e)))
}

impl Exporter for ParquetExporter {
    fn format_name(&self) -> &'static str {
        "parquet"
    }

    fn file_extension(&self) -> &'static str {
        "parquet"
    }

    fn export(&self, events: &[DebugEvent], writer: &mut dyn Write) -> Result<(), ExportError> {
        if events.is_empty() {
            // Write an empty Parquet file with the schema
            let schema = create_schema();
            let batch = RecordBatch::new_empty(schema);
            let props = WriterProperties::builder().build();

            // Arrow writer requires a file-like object with seek, but we have a Write trait.
            // We'll need to collect to a buffer first, then write.
            let mut buffer = Vec::new();
            let mut arrow_writer =
                ArrowWriter::try_new(&mut buffer, batch.schema(), Some(props))
                    .map_err(|e| ExportError::Other(format!("Parquet writer error: {}", e)))?;
            arrow_writer
                .write(&batch)
                .map_err(|e| ExportError::Other(format!("Parquet write error: {}", e)))?;
            arrow_writer
                .close()
                .map_err(|e| ExportError::Other(format!("Parquet close error: {}", e)))?;
            writer.write_all(&buffer)?;
            return Ok(());
        }

        let batch = events_to_record_batch(events)?;
        let props = WriterProperties::builder().build();

        let mut buffer = Vec::new();
        let mut arrow_writer = ArrowWriter::try_new(&mut buffer, batch.schema(), Some(props))
            .map_err(|e| ExportError::Other(format!("Parquet writer error: {}", e)))?;
        arrow_writer
            .write(&batch)
            .map_err(|e| ExportError::Other(format!("Parquet write error: {}", e)))?;
        arrow_writer
            .close()
            .map_err(|e| ExportError::Other(format!("Parquet close error: {}", e)))?;

        writer.write_all(&buffer)?;
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
            .payload(Payload::Decoded {
                raw: Bytes::from_static(b"hello"),
                fields: serde_json::json!({"user_id": "abc-123"}),
                schema_name: Some("User".into()),
            })
            .metadata("grpc.method", "/api.v1.Users/Get")
            .build()
    }

    #[test]
    fn parquet_schema() {
        let schema = create_schema();
        assert_eq!(schema.fields().len(), 20);
        assert_eq!(schema.field(0).name(), "id");
        assert_eq!(schema.field(7).name(), "transport");
    }

    #[test]
    fn parquet_round_trip() {
        let events = vec![sample_event()];
        let mut buf = Vec::new();
        ParquetExporter.export(&events, &mut buf).unwrap();
        assert!(!buf.is_empty());

        // Verify it's a valid Parquet file by reading back the schema
        use parquet::file::reader::SerializedFileReader;
        use std::io::Cursor;

        let cursor = Cursor::new(buf);
        let reader = SerializedFileReader::new(cursor).unwrap();
        let metadata = reader.metadata();
        assert_eq!(metadata.num_row_groups(), 1);
    }

    #[test]
    fn parquet_empty_events() {
        let mut buf = Vec::new();
        ParquetExporter.export(&[], &mut buf).unwrap();
        assert!(!buf.is_empty());
    }
}

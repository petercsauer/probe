use crate::{ExportError, Exporter};
use prb_core::{
    CorrelationKey, DebugEvent, Direction, METADATA_KEY_OTEL_PARENT_SPAN_ID,
    METADATA_KEY_OTEL_SPAN_ID, METADATA_KEY_OTEL_TRACE_ID,
};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;

pub struct OtlpExporter;

// Minimal OTLP JSON types per the OpenTelemetry protobuf spec.
// We define these manually to avoid pulling in the full opentelemetry-proto crate.

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportTraceServiceRequest {
    resource_spans: Vec<ResourceSpans>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceSpans {
    resource: Resource,
    scope_spans: Vec<ScopeSpans>,
}

#[derive(Serialize)]
struct Resource {
    attributes: Vec<KeyValue>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScopeSpans {
    scope: InstrumentationScope,
    spans: Vec<Span>,
}

#[derive(Serialize)]
struct InstrumentationScope {
    name: String,
    version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Span {
    trace_id: String,
    span_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_span_id: Option<String>,
    name: String,
    kind: i32,
    #[serde(serialize_with = "serialize_nanos_as_string")]
    start_time_unix_nano: u64,
    #[serde(serialize_with = "serialize_nanos_as_string")]
    end_time_unix_nano: u64,
    attributes: Vec<KeyValue>,
    status: SpanStatus,
}

#[derive(Serialize)]
struct KeyValue {
    key: String,
    value: AnyValue,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AnyValue {
    #[serde(skip_serializing_if = "Option::is_none")]
    string_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    int_value: Option<String>,
}

#[derive(Serialize)]
struct SpanStatus {
    code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

fn serialize_nanos_as_string<S>(nanos: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&nanos.to_string())
}

const SPAN_KIND_INTERNAL: i32 = 1;
const SPAN_KIND_SERVER: i32 = 2;
const SPAN_KIND_CLIENT: i32 = 3;

const STATUS_CODE_UNSET: i32 = 0;
const STATUS_CODE_OK: i32 = 1;
const STATUS_CODE_ERROR: i32 = 2;

fn string_attr(key: &str, value: &str) -> KeyValue {
    KeyValue {
        key: key.into(),
        value: AnyValue {
            string_value: Some(value.into()),
            int_value: None,
        },
    }
}

fn int_attr(key: &str, value: i64) -> KeyValue {
    KeyValue {
        key: key.into(),
        value: AnyValue {
            string_value: None,
            int_value: Some(value.to_string()),
        },
    }
}

fn deterministic_trace_id(event: &DebugEvent) -> String {
    // Use correlation keys to group events into the same trace.
    // Falls back to a deterministic ID from the event's connection info.
    for key in &event.correlation_keys {
        if let CorrelationKey::ConnectionId { id } = key {
            let hash = simple_hash(id.as_bytes());
            return format!("{hash:032x}");
        }
    }
    if let Some(ref net) = event.source.network {
        let key = format!("{}->{}", net.src, net.dst);
        let hash = simple_hash(key.as_bytes());
        return format!("{hash:032x}");
    }
    let hash = simple_hash(event.source.origin.as_bytes());
    format!("{hash:032x}")
}

fn deterministic_span_id(event: &DebugEvent) -> String {
    let hash = simple_hash(&event.id.as_u64().to_le_bytes());
    format!("{:016x}", hash & 0xFFFF_FFFF_FFFF_FFFF)
}

fn simple_hash(data: &[u8]) -> u128 {
    // FNV-1a 128-bit
    let mut hash: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    for &byte in data {
        hash ^= u128::from(byte);
        hash = hash.wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_013B);
    }
    hash
}

fn event_to_span(event: &DebugEvent) -> Span {
    let name = event
        .metadata
        .get("grpc.method")
        .cloned()
        .unwrap_or_else(|| format!("{} {}", event.transport, event.direction));

    let kind = match event.direction {
        Direction::Outbound => SPAN_KIND_CLIENT,
        Direction::Inbound => SPAN_KIND_SERVER,
        Direction::Unknown => SPAN_KIND_INTERNAL,
    };

    let mut attributes = vec![
        string_attr("probe.transport", &event.transport.to_string()),
        string_attr("probe.direction", &event.direction.to_string()),
        string_attr("probe.adapter", &event.source.adapter),
        string_attr("probe.origin", &event.source.origin),
        int_attr("probe.event_id", event.id.as_u64() as i64),
    ];

    if let Some(ref net) = event.source.network {
        attributes.push(string_attr("net.peer.ip", &net.src));
        attributes.push(string_attr("net.host.ip", &net.dst));
    }

    for (key, value) in &event.metadata {
        // Skip otel.* keys as they're already in trace_id/span_id/parent_span_id
        if !key.starts_with("otel.") {
            attributes.push(string_attr(&format!("probe.metadata.{key}"), value));
        }
    }

    if let Some(seq) = event.sequence {
        attributes.push(int_attr("probe.sequence", seq as i64));
    }

    for warning in &event.warnings {
        attributes.push(string_attr("probe.warning", warning));
    }

    let status = match event
        .metadata
        .get("grpc.status")
        .map(std::string::String::as_str)
    {
        Some("0") => SpanStatus {
            code: STATUS_CODE_OK,
            message: None,
        },
        Some(code) => SpanStatus {
            code: STATUS_CODE_ERROR,
            message: Some(format!("gRPC status {code}")),
        },
        None if !event.warnings.is_empty() => SpanStatus {
            code: STATUS_CODE_ERROR,
            message: event.warnings.first().cloned(),
        },
        None => SpanStatus {
            code: STATUS_CODE_UNSET,
            message: None,
        },
    };

    let ts = event.timestamp.as_nanos();

    // Use actual trace context from metadata if available, otherwise fall back to deterministic IDs
    let trace_id = event
        .metadata
        .get(METADATA_KEY_OTEL_TRACE_ID)
        .cloned()
        .unwrap_or_else(|| deterministic_trace_id(event));

    let span_id = event
        .metadata
        .get(METADATA_KEY_OTEL_SPAN_ID)
        .cloned()
        .unwrap_or_else(|| deterministic_span_id(event));

    let parent_span_id = event
        .metadata
        .get(METADATA_KEY_OTEL_PARENT_SPAN_ID)
        .cloned();

    Span {
        trace_id,
        span_id,
        parent_span_id,
        name,
        kind,
        start_time_unix_nano: ts,
        end_time_unix_nano: ts,
        attributes,
        status,
    }
}

impl Exporter for OtlpExporter {
    fn format_name(&self) -> &'static str {
        "otlp"
    }

    fn file_extension(&self) -> &'static str {
        "json"
    }

    fn export(&self, events: &[DebugEvent], writer: &mut dyn Write) -> Result<(), ExportError> {
        // Group events by adapter/origin to create separate ResourceSpans
        let mut groups: HashMap<String, Vec<&DebugEvent>> = HashMap::new();
        for event in events {
            let key = format!("{}:{}", event.source.adapter, event.source.origin);
            groups.entry(key).or_default().push(event);
        }

        let resource_spans: Vec<ResourceSpans> = groups
            .into_values()
            .map(|group_events| {
                let first = group_events[0];
                let resource = Resource {
                    attributes: vec![
                        string_attr("service.name", "probe"),
                        string_attr("probe.adapter", &first.source.adapter),
                        string_attr("probe.origin", &first.source.origin),
                    ],
                };

                let spans: Vec<Span> = group_events.iter().map(|e| event_to_span(e)).collect();

                ResourceSpans {
                    resource,
                    scope_spans: vec![ScopeSpans {
                        scope: InstrumentationScope {
                            name: "prb-export".into(),
                            version: env!("CARGO_PKG_VERSION").into(),
                        },
                        spans,
                    }],
                }
            })
            .collect();

        let request = ExportTraceServiceRequest { resource_spans };
        serde_json::to_writer_pretty(writer, &request)?;
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
            .id(EventId::from_raw(42))
            .timestamp(Timestamp::from_nanos(1_710_000_000_000_000_000))
            .source(EventSource {
                adapter: "pcap".into(),
                origin: "test.pcap".into(),
                network: Some(NetworkAddr {
                    src: "10.0.0.1:42837".into(),
                    dst: "10.0.0.2:50051".into(),
                }),
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"hello"),
            })
            .metadata("grpc.method", "/api.v1.Users/GetUser")
            .metadata("grpc.status", "0")
            .build()
    }

    #[test]
    fn otlp_valid_json() {
        let events = vec![sample_event()];
        let mut buf = Vec::new();
        OtlpExporter.export(&events, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed["resourceSpans"].is_array());
        let rs = &parsed["resourceSpans"][0];
        assert!(rs["scopeSpans"][0]["spans"].is_array());
    }

    #[test]
    fn otlp_span_attributes() {
        let events = vec![sample_event()];
        let mut buf = Vec::new();
        OtlpExporter.export(&events, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let span = &parsed["resourceSpans"][0]["scopeSpans"][0]["spans"][0];

        assert_eq!(span["name"], "/api.v1.Users/GetUser");
        assert_eq!(span["kind"], SPAN_KIND_CLIENT);
        assert_eq!(span["status"]["code"], STATUS_CODE_OK);

        // Verify trace_id and span_id are hex strings of correct length
        let trace_id = span["traceId"].as_str().unwrap();
        let span_id = span["spanId"].as_str().unwrap();
        assert_eq!(trace_id.len(), 32);
        assert_eq!(span_id.len(), 16);
    }

    #[test]
    fn otlp_nanos_as_strings() {
        let events = vec![sample_event()];
        let mut buf = Vec::new();
        OtlpExporter.export(&events, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let span = &parsed["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
        assert!(span["startTimeUnixNano"].is_string());
        assert_eq!(span["startTimeUnixNano"], "1710000000000000000");
    }
}

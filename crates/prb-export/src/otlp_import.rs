use crate::ExportError;
use prb_core::{DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind, CorrelationKey};
use prb_core::{METADATA_KEY_OTEL_TRACE_ID, METADATA_KEY_OTEL_SPAN_ID, METADATA_KEY_OTEL_PARENT_SPAN_ID};
use serde::Deserialize;
use std::collections::BTreeMap;

/// Top-level OTLP trace export request (minimal subset for import).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTraceServiceRequest {
    pub resource_spans: Vec<ResourceSpans>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSpans {
    pub resource: Resource,
    pub scope_spans: Vec<ScopeSpans>,
}

#[derive(Deserialize)]
pub struct Resource {
    pub attributes: Vec<KeyValue>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeSpans {
    pub scope: InstrumentationScope,
    pub spans: Vec<Span>,
}

#[derive(Deserialize)]
pub struct InstrumentationScope {
    pub name: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub trace_id: String,
    pub span_id: String,
    #[serde(default)]
    pub parent_span_id: Option<String>,
    pub name: String,
    pub kind: i32,
    #[serde(deserialize_with = "deserialize_nanos_from_string")]
    pub start_time_unix_nano: u64,
    #[serde(deserialize_with = "deserialize_nanos_from_string")]
    pub end_time_unix_nano: u64,
    #[serde(default)]
    pub attributes: Vec<KeyValue>,
    #[serde(default)]
    pub status: Option<SpanStatus>,
}

#[derive(Deserialize)]
pub struct KeyValue {
    pub key: String,
    pub value: AnyValue,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnyValue {
    pub string_value: Option<String>,
    pub int_value: Option<String>,
    pub bool_value: Option<bool>,
    pub double_value: Option<f64>,
}

#[derive(Deserialize)]
pub struct SpanStatus {
    pub code: i32,
    pub message: Option<String>,
}

fn deserialize_nanos_from_string<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let s = String::deserialize(deserializer)?;
    s.parse::<u64>().map_err(D::Error::custom)
}

/// Parse OTLP JSON from bytes.
pub fn parse_otlp_json(data: &[u8]) -> Result<ExportTraceServiceRequest, ExportError> {
    serde_json::from_slice(data).map_err(|e| ExportError::SerializationError(e.to_string()))
}

/// Convert OTLP spans to DebugEvents.
pub fn otlp_to_events(request: &ExportTraceServiceRequest) -> Vec<DebugEvent> {
    let mut events = Vec::new();
    let mut event_id_counter = 1u64;

    for resource_spans in &request.resource_spans {
        // Extract service name from resource attributes
        let service_name = resource_spans
            .resource
            .attributes
            .iter()
            .find(|kv| kv.key == "service.name")
            .and_then(|kv| kv.value.string_value.clone())
            .unwrap_or_else(|| "unknown".to_string());

        for scope_spans in &resource_spans.scope_spans {
            for span in &scope_spans.spans {
                let event = span_to_event(span, &service_name, event_id_counter);
                events.push(event);
                event_id_counter += 1;
            }
        }
    }

    events
}

fn span_to_event(span: &Span, service_name: &str, event_id: u64) -> DebugEvent {
    // Infer direction from span kind
    let direction = match span.kind {
        1 => Direction::Unknown, // INTERNAL
        2 => Direction::Inbound,  // SERVER
        3 => Direction::Outbound, // CLIENT
        4 => Direction::Outbound, // PRODUCER
        5 => Direction::Inbound,  // CONSUMER
        _ => Direction::Unknown,
    };

    // Build metadata from span attributes
    let mut metadata = BTreeMap::new();
    metadata.insert(METADATA_KEY_OTEL_TRACE_ID.to_string(), span.trace_id.clone());
    metadata.insert(METADATA_KEY_OTEL_SPAN_ID.to_string(), span.span_id.clone());

    if let Some(ref parent_span_id) = span.parent_span_id {
        metadata.insert(METADATA_KEY_OTEL_PARENT_SPAN_ID.to_string(), parent_span_id.clone());
    }

    // Add span attributes to metadata
    for kv in &span.attributes {
        let value = if let Some(ref s) = kv.value.string_value {
            s.clone()
        } else if let Some(ref i) = kv.value.int_value {
            i.clone()
        } else if let Some(b) = kv.value.bool_value {
            b.to_string()
        } else if let Some(d) = kv.value.double_value {
            d.to_string()
        } else {
            continue;
        };

        // Strip probe.metadata. prefix if present
        let key = if kv.key.starts_with("probe.metadata.") {
            kv.key.strip_prefix("probe.metadata.").unwrap().to_string()
        } else {
            kv.key.clone()
        };

        metadata.insert(key, value);
    }

    // Extract network info if present
    let network = if let (Some(src), Some(dst)) = (
        span.attributes.iter().find(|kv| kv.key == "net.peer.ip"),
        span.attributes.iter().find(|kv| kv.key == "net.host.ip"),
    ) {
        Some(NetworkAddr {
            src: src.value.string_value.clone().unwrap_or_default(),
            dst: dst.value.string_value.clone().unwrap_or_default(),
        })
    } else {
        None
    };

    // Determine transport
    let transport = metadata
        .get("probe.transport")
        .and_then(|s| s.parse::<TransportKind>().ok())
        .unwrap_or(TransportKind::Grpc);

    let mut builder = DebugEvent::builder()
        .id(EventId::from_raw(event_id))
        .timestamp(Timestamp::from_nanos(span.start_time_unix_nano))
        .source(EventSource {
            adapter: "otlp-import".to_string(),
            origin: service_name.to_string(),
            network,
        })
        .transport(transport)
        .direction(direction)
        .payload(Payload::Raw {
            raw: bytes::Bytes::from(span.name.clone()),
        })
        .correlation_key(CorrelationKey::TraceContext {
            trace_id: span.trace_id.clone(),
            span_id: span.span_id.clone(),
        });

    // Add all metadata entries
    for (key, value) in metadata {
        builder = builder.metadata(key, value);
    }

    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_otlp_json_minimal() {
        let json = r#"{
            "resourceSpans": [{
                "resource": {
                    "attributes": [
                        {"key": "service.name", "value": {"stringValue": "test-service"}}
                    ]
                },
                "scopeSpans": [{
                    "scope": {"name": "test-scope", "version": "1.0"},
                    "spans": [{
                        "traceId": "4bf92f3577b34da6a3ce929d0e0e4736",
                        "spanId": "00f067aa0ba902b7",
                        "name": "/api.v1.Users/Get",
                        "kind": 3,
                        "startTimeUnixNano": "1710000000000000000",
                        "endTimeUnixNano": "1710000000100000000",
                        "attributes": []
                    }]
                }]
            }]
        }"#;

        let request = parse_otlp_json(json.as_bytes()).unwrap();
        assert_eq!(request.resource_spans.len(), 1);
        assert_eq!(request.resource_spans[0].scope_spans[0].spans.len(), 1);

        let span = &request.resource_spans[0].scope_spans[0].spans[0];
        assert_eq!(span.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(span.span_id, "00f067aa0ba902b7");
        assert_eq!(span.name, "/api.v1.Users/Get");
    }

    #[test]
    fn test_otlp_to_events() {
        let json = r#"{
            "resourceSpans": [{
                "resource": {
                    "attributes": [
                        {"key": "service.name", "value": {"stringValue": "test-service"}}
                    ]
                },
                "scopeSpans": [{
                    "scope": {"name": "test-scope", "version": "1.0"},
                    "spans": [{
                        "traceId": "4bf92f3577b34da6a3ce929d0e0e4736",
                        "spanId": "00f067aa0ba902b7",
                        "name": "/api.v1.Users/Get",
                        "kind": 3,
                        "startTimeUnixNano": "1710000000000000000",
                        "endTimeUnixNano": "1710000000100000000",
                        "attributes": [
                            {"key": "grpc.method", "value": {"stringValue": "/api.v1.Users/Get"}}
                        ]
                    }]
                }]
            }]
        }"#;

        let request = parse_otlp_json(json.as_bytes()).unwrap();
        let events = otlp_to_events(&request);

        assert_eq!(events.len(), 1);
        let event = &events[0];

        assert_eq!(event.source.adapter, "otlp-import");
        assert_eq!(event.source.origin, "test-service");
        assert_eq!(event.direction, Direction::Outbound);
        assert_eq!(event.metadata.get(METADATA_KEY_OTEL_TRACE_ID).unwrap(), "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(event.metadata.get(METADATA_KEY_OTEL_SPAN_ID).unwrap(), "00f067aa0ba902b7");
    }

    #[test]
    fn test_span_kind_to_direction() {
        let json = r#"{
            "resourceSpans": [{
                "resource": {
                    "attributes": [
                        {"key": "service.name", "value": {"stringValue": "test"}}
                    ]
                },
                "scopeSpans": [{
                    "scope": {"name": "test", "version": "1.0"},
                    "spans": [
                        {
                            "traceId": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                            "spanId": "bbbbbbbbbbbbbbbb",
                            "name": "client",
                            "kind": 3,
                            "startTimeUnixNano": "1710000000000000000",
                            "endTimeUnixNano": "1710000000000000000",
                            "attributes": []
                        },
                        {
                            "traceId": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                            "spanId": "cccccccccccccccc",
                            "name": "server",
                            "kind": 2,
                            "startTimeUnixNano": "1710000000000000000",
                            "endTimeUnixNano": "1710000000000000000",
                            "attributes": []
                        }
                    ]
                }]
            }]
        }"#;

        let request = parse_otlp_json(json.as_bytes()).unwrap();
        let events = otlp_to_events(&request);

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].direction, Direction::Outbound); // CLIENT
        assert_eq!(events[1].direction, Direction::Inbound);  // SERVER
    }
}

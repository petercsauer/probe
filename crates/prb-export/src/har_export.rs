use crate::{ExportError, Exporter};
use har::v1_2::{self};
use prb_core::{DebugEvent, Payload, Timestamp, TransportKind};
use std::io::Write;

pub struct HarExporter;

fn timestamp_to_iso8601(ts: Timestamp) -> String {
    let nanos = ts.as_nanos();
    let secs = (nanos / 1_000_000_000) as i64;
    let subsec_nanos = (nanos % 1_000_000_000) as u32;
    chrono::DateTime::from_timestamp(secs, subsec_nanos).map_or_else(
        || "1970-01-01T00:00:00.000Z".into(),
        |dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    )
}

const fn payload_size(event: &DebugEvent) -> i64 {
    match &event.payload {
        Payload::Raw { raw } => raw.len() as i64,
        Payload::Decoded { raw, .. } => raw.len() as i64,
    }
}

fn payload_body_text(event: &DebugEvent) -> Option<String> {
    match &event.payload {
        Payload::Decoded { fields, .. } => Some(serde_json::to_string_pretty(fields).ok()?),
        Payload::Raw { raw } if !raw.is_empty() => String::from_utf8(raw.to_vec()).ok(),
        _ => None,
    }
}

fn event_to_har_entry(event: &DebugEvent) -> v1_2::Entries {
    let method = event
        .metadata
        .get("grpc.method")
        .cloned()
        .unwrap_or_else(|| "POST".into());

    let authority = event
        .metadata
        .get("grpc.authority")
        .cloned()
        .or_else(|| event.source.network.as_ref().map(|n| n.dst.clone()))
        .unwrap_or_else(|| "unknown".into());

    let url = format!("https://{authority}{method}");

    let mut request_headers = vec![v1_2::Headers {
        name: "content-type".into(),
        value: "application/grpc".into(),
        comment: None,
    }];

    if let Some(encoding) = event.metadata.get("grpc.encoding") {
        request_headers.push(v1_2::Headers {
            name: "grpc-encoding".into(),
            value: encoding.clone(),
            comment: None,
        });
    }

    let post_data = payload_body_text(event).map(|text| v1_2::PostData {
        mime_type: "application/grpc+proto".into(),
        text: Some(text),
        params: None,
        comment: None,
    });

    let body_size = payload_size(event);

    let server_ip = event
        .source
        .network
        .as_ref()
        .map(|n| n.dst.split(':').next().unwrap_or(&n.dst).to_string());

    v1_2::Entries {
        pageref: None,
        started_date_time: timestamp_to_iso8601(event.timestamp),
        time: 0.0,
        request: v1_2::Request {
            method: "POST".into(),
            url,
            http_version: "HTTP/2.0".into(),
            cookies: vec![],
            headers: request_headers,
            query_string: vec![],
            post_data,
            headers_size: -1,
            body_size,
            comment: None,
        },
        response: v1_2::Response {
            status: grpc_status_to_http(event),
            status_text: grpc_status_text(event),
            http_version: "HTTP/2.0".into(),
            cookies: vec![],
            headers: grpc_response_headers(event),
            content: v1_2::Content {
                size: body_size,
                compression: None,
                mime_type: Some("application/grpc".into()),
                text: None,
                encoding: None,
                comment: None,
            },
            redirect_url: None,
            headers_size: -1,
            body_size: 0,
            comment: None,
        },
        cache: v1_2::Cache {
            before_request: None,
            after_request: None,
        },
        timings: v1_2::Timings {
            blocked: None,
            dns: None,
            connect: None,
            send: 0.0,
            wait: 0.0,
            receive: 0.0,
            ssl: None,
            comment: None,
        },
        server_ip_address: server_ip,
        connection: event
            .source
            .network
            .as_ref()
            .map(|n| format!("{}->{}", n.src, n.dst)),
        comment: Some(format!("Probe event #{} ({})", event.id, event.transport)),
    }
}

fn grpc_status_to_http(event: &DebugEvent) -> i64 {
    match event
        .metadata
        .get("grpc.status")
        .map(std::string::String::as_str)
    {
        Some("0") | None => 200,
        Some("1") => 499,  // CANCELLED
        Some("3") => 400,  // INVALID_ARGUMENT
        Some("4") => 504,  // DEADLINE_EXCEEDED
        Some("5") => 404,  // NOT_FOUND
        Some("7") => 403,  // PERMISSION_DENIED
        Some("12") => 501, // UNIMPLEMENTED
        Some("13") => 500, // INTERNAL
        Some("14") => 503, // UNAVAILABLE
        Some("16") => 401, // UNAUTHENTICATED
        _ => 500,
    }
}

fn grpc_status_text(event: &DebugEvent) -> String {
    match event
        .metadata
        .get("grpc.status")
        .map(std::string::String::as_str)
    {
        Some("0") | None => "OK".into(),
        Some("1") => "Cancelled".into(),
        Some("2") => "Unknown".into(),
        Some("3") => "Invalid Argument".into(),
        Some("4") => "Deadline Exceeded".into(),
        Some("5") => "Not Found".into(),
        Some("7") => "Permission Denied".into(),
        Some("12") => "Unimplemented".into(),
        Some("13") => "Internal".into(),
        Some("14") => "Unavailable".into(),
        Some("16") => "Unauthenticated".into(),
        Some(other) => format!("gRPC status {other}"),
    }
}

fn grpc_response_headers(event: &DebugEvent) -> Vec<v1_2::Headers> {
    let mut headers = vec![v1_2::Headers {
        name: "content-type".into(),
        value: "application/grpc".into(),
        comment: None,
    }];

    if let Some(status) = event.metadata.get("grpc.status") {
        headers.push(v1_2::Headers {
            name: "grpc-status".into(),
            value: status.clone(),
            comment: None,
        });
    }
    if let Some(msg) = event.metadata.get("grpc.message") {
        headers.push(v1_2::Headers {
            name: "grpc-message".into(),
            value: msg.clone(),
            comment: None,
        });
    }

    headers
}

impl Exporter for HarExporter {
    fn format_name(&self) -> &'static str {
        "har"
    }

    fn file_extension(&self) -> &'static str {
        "har"
    }

    fn export(&self, events: &[DebugEvent], writer: &mut dyn Write) -> Result<(), ExportError> {
        let grpc_events: Vec<_> = events
            .iter()
            .filter(|e| e.transport == TransportKind::Grpc)
            .collect();

        let entries: Vec<_> = grpc_events.iter().map(|e| event_to_har_entry(e)).collect();

        let non_grpc_count = events.len() - grpc_events.len();
        let comment = if non_grpc_count > 0 {
            Some(format!(
                "Exported by Probe. {non_grpc_count} non-HTTP events (ZMQ/DDS/TCP/UDP) omitted."
            ))
        } else {
            None
        };

        let log = v1_2::Log {
            creator: v1_2::Creator {
                name: "probe".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                comment: Some("Universal message debugger for gRPC, ZMQ, and DDS".into()),
            },
            browser: None,
            pages: None,
            entries,
            comment,
        };

        let har_spec = har::Har {
            log: har::Spec::V1_2(log),
        };
        let json = har::to_json(&har_spec).map_err(|e| ExportError::Other(e.to_string()))?;
        writer.write_all(json.as_bytes())?;
        writer.write_all(b"\n")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use prb_core::*;

    fn grpc_event() -> DebugEvent {
        DebugEvent::builder()
            .id(EventId::from_raw(1))
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
                raw: Bytes::from_static(b"\x00\x00\x00\x00\x05hello"),
            })
            .metadata("grpc.method", "/api.v1.Users/GetUser")
            .metadata("grpc.status", "0")
            .metadata("h2.stream_id", "1")
            .build()
    }

    fn zmq_event() -> DebugEvent {
        DebugEvent::builder()
            .id(EventId::from_raw(2))
            .timestamp(Timestamp::from_nanos(1_710_000_001_000_000_000))
            .source(EventSource {
                adapter: "pcap".into(),
                origin: "test.pcap".into(),
                network: None,
            })
            .transport(TransportKind::Zmq)
            .direction(Direction::Inbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"data"),
            })
            .build()
    }

    #[test]
    fn har_grpc_events() {
        let events = vec![grpc_event()];
        let mut buf = Vec::new();
        HarExporter.export(&events, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let log = &parsed["log"];
        assert_eq!(log["creator"]["name"], "probe");
        assert_eq!(log["entries"].as_array().unwrap().len(), 1);

        let entry = &log["entries"][0];
        assert!(
            entry["request"]["url"]
                .as_str()
                .unwrap()
                .contains("/api.v1.Users/GetUser")
        );
        assert_eq!(entry["request"]["method"], "POST");
        assert_eq!(entry["request"]["httpVersion"], "HTTP/2.0");
        assert_eq!(entry["response"]["status"], 200);
    }

    #[test]
    fn har_skips_non_http() {
        let events = vec![grpc_event(), zmq_event()];
        let mut buf = Vec::new();
        HarExporter.export(&events, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let entries = parsed["log"]["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 1);

        let comment = parsed["log"]["comment"].as_str().unwrap();
        assert!(comment.contains("1 non-HTTP events"));
    }

    #[test]
    fn har_large_payload() {
        let large_data = vec![0u8; 100_000];
        let event = DebugEvent::builder()
            .id(EventId::from_raw(1))
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
                raw: Bytes::from(large_data),
            })
            .metadata("grpc.method", "/test/LargeMethod")
            .build();

        let mut buf = Vec::new();
        HarExporter.export(&[event], &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let entry = &parsed["log"]["entries"][0];
        assert_eq!(entry["request"]["bodySize"], 100_000);
    }

    #[test]
    fn har_grpc_error_status() {
        let error_event = DebugEvent::builder()
            .id(EventId::from_raw(1))
            .timestamp(Timestamp::from_nanos(1_710_000_000_000_000_000))
            .source(EventSource {
                adapter: "pcap".into(),
                origin: "test.pcap".into(),
                network: None,
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Inbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"error"),
            })
            .metadata("grpc.method", "/api/FailMethod")
            .metadata("grpc.status", "5")
            .metadata("grpc.message", "Not Found")
            .build();

        let mut buf = Vec::new();
        HarExporter.export(&[error_event], &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let entry = &parsed["log"]["entries"][0];
        assert_eq!(entry["response"]["status"], 404);
        assert_eq!(entry["response"]["statusText"], "Not Found");

        let headers = entry["response"]["headers"].as_array().unwrap();
        let grpc_status_header = headers.iter().find(|h| h["name"] == "grpc-status").unwrap();
        assert_eq!(grpc_status_header["value"], "5");
    }

    #[test]
    fn har_tls_decrypted_event() {
        let tls_event = DebugEvent::builder()
            .id(EventId::from_raw(1))
            .timestamp(Timestamp::from_nanos(1_710_000_000_000_000_000))
            .source(EventSource {
                adapter: "pcap".into(),
                origin: "test.pcap".into(),
                network: Some(NetworkAddr {
                    src: "10.0.0.1:42837".into(),
                    dst: "10.0.0.2:443".into(),
                }),
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"decrypted"),
            })
            .metadata("grpc.method", "/api/SecureMethod")
            .metadata("tls.version", "TLS 1.3")
            .metadata("tls.cipher_suite", "TLS_AES_256_GCM_SHA384")
            .build();

        let mut buf = Vec::new();
        HarExporter.export(&[tls_event], &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let entry = &parsed["log"]["entries"][0];
        assert!(
            entry["request"]["url"]
                .as_str()
                .unwrap()
                .contains("SecureMethod")
        );
        // Timings include ssl field (can be -1 or null for unavailable)
        assert!(entry["timings"]["ssl"].is_null());
    }

    #[test]
    fn har_empty_payload() {
        let empty_event = DebugEvent::builder()
            .id(EventId::from_raw(1))
            .timestamp(Timestamp::from_nanos(1_710_000_000_000_000_000))
            .source(EventSource {
                adapter: "pcap".into(),
                origin: "test.pcap".into(),
                network: None,
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Outbound)
            .payload(Payload::Raw { raw: Bytes::new() })
            .metadata("grpc.method", "/api/EmptyMethod")
            .build();

        let mut buf = Vec::new();
        HarExporter.export(&[empty_event], &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let entry = &parsed["log"]["entries"][0];
        assert_eq!(entry["request"]["bodySize"], 0);
    }

    #[test]
    fn har_various_grpc_status_codes() {
        let statuses = vec![
            ("1", 499, "Cancelled"),
            ("3", 400, "Invalid Argument"),
            ("4", 504, "Deadline Exceeded"),
            ("7", 403, "Permission Denied"),
            ("12", 501, "Unimplemented"),
            ("13", 500, "Internal"),
            ("14", 503, "Unavailable"),
            ("16", 401, "Unauthenticated"),
        ];

        for (grpc_code, expected_http, expected_text) in statuses {
            let event = DebugEvent::builder()
                .id(EventId::from_raw(1))
                .timestamp(Timestamp::from_nanos(1_710_000_000_000_000_000))
                .source(EventSource {
                    adapter: "pcap".into(),
                    origin: "test.pcap".into(),
                    network: None,
                })
                .transport(TransportKind::Grpc)
                .direction(Direction::Outbound)
                .payload(Payload::Raw {
                    raw: Bytes::from_static(b"test"),
                })
                .metadata("grpc.method", "/api/Test")
                .metadata("grpc.status", grpc_code)
                .build();

            let mut buf = Vec::new();
            HarExporter.export(&[event], &mut buf).unwrap();
            let output = String::from_utf8(buf).unwrap();

            let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
            let entry = &parsed["log"]["entries"][0];
            assert_eq!(
                entry["response"]["status"], expected_http,
                "gRPC status {grpc_code} should map to HTTP {expected_http}"
            );
            assert_eq!(entry["response"]["statusText"], expected_text);
        }
    }
}

use prb_core::{
    DebugEvent, Direction, METADATA_KEY_DDS_DOMAIN_ID, METADATA_KEY_DDS_TOPIC_NAME,
    METADATA_KEY_GRPC_METHOD, METADATA_KEY_H2_STREAM_ID, METADATA_KEY_ZMQ_TOPIC, Payload,
    TransportKind,
};

/// Structured context for AI explanation, built from DebugEvents.
#[derive(Debug, Clone)]
pub struct ExplainContext {
    pub target_summary: String,
    pub surrounding_summaries: Vec<String>,
    pub transport: TransportKind,
    pub has_errors: bool,
    pub has_warnings: bool,
}

impl ExplainContext {
    /// Build context from events with a target event index.
    pub fn build(events: &[DebugEvent], target_idx: usize, context_window: usize) -> Self {
        let target = &events[target_idx];
        let target_summary = summarize_event(target);

        let start = target_idx.saturating_sub(context_window);
        let end = (target_idx + context_window + 1).min(events.len());

        let surrounding_summaries: Vec<String> = events[start..end]
            .iter()
            .enumerate()
            .filter(|(i, _)| start + i != target_idx)
            .map(|(_, e)| summarize_event(e))
            .collect();

        let has_errors = target.metadata.iter().any(|(k, v)| {
            (k.contains("status") && v != "0" && v != "OK")
                || k.contains("error")
                || (k == "grpc.status" && v != "0")
        });

        let has_warnings = !target.warnings.is_empty();

        ExplainContext {
            target_summary,
            surrounding_summaries,
            transport: target.transport,
            has_errors,
            has_warnings,
        }
    }

    /// Find the index of an event by its ID.
    pub fn find_event_index(events: &[DebugEvent], event_id: u64) -> Option<usize> {
        events.iter().position(|e| e.id.as_u64() == event_id)
    }
}

fn summarize_event(event: &DebugEvent) -> String {
    let mut parts = Vec::new();

    parts.push(format!("[Event {}]", event.id));
    parts.push(format!(
        "Timestamp: {}",
        format_timestamp(event.timestamp.as_nanos())
    ));
    parts.push(format!("Transport: {}", event.transport));
    parts.push(format!("Direction: {}", event.direction));

    if let Some(ref network) = event.source.network {
        parts.push(format!("From: {} → To: {}", network.src, network.dst));
    }

    match event.transport {
        TransportKind::Grpc => summarize_grpc(event, &mut parts),
        TransportKind::Zmq => summarize_zmq(event, &mut parts),
        TransportKind::DdsRtps => summarize_dds(event, &mut parts),
        _ => summarize_generic(event, &mut parts),
    }

    if let Payload::Decoded {
        ref fields,
        ref schema_name,
        ..
    } = event.payload
    {
        if let Some(name) = schema_name {
            parts.push(format!("Schema: {name}"));
        }
        let fields_str = serde_json::to_string(fields).unwrap_or_default();
        if fields_str.len() <= 500 {
            parts.push(format!("Decoded fields: {fields_str}"));
        } else {
            parts.push(format!(
                "Decoded fields (truncated): {}...",
                &fields_str[..500]
            ));
        }
    } else if let Payload::Raw { ref raw } = event.payload {
        parts.push(format!("Payload size: {} bytes", raw.len()));
        if let Ok(text) = std::str::from_utf8(raw) {
            let preview = if text.len() > 200 { &text[..200] } else { text };
            parts.push(format!("Payload (UTF-8): {preview}"));
        }
    }

    if !event.warnings.is_empty() {
        parts.push(format!("[!] Warnings: {}", event.warnings.join("; ")));
    }

    for (k, v) in &event.metadata {
        if !is_already_summarized(k) {
            parts.push(format!("{k}: {v}"));
        }
    }

    parts.join("\n")
}

fn summarize_grpc(event: &DebugEvent, parts: &mut Vec<String>) {
    if let Some(method) = event.metadata.get(METADATA_KEY_GRPC_METHOD) {
        parts.push(format!("gRPC method: {method}"));
    }
    if let Some(stream_id) = event.metadata.get(METADATA_KEY_H2_STREAM_ID) {
        parts.push(format!("HTTP/2 stream ID: {stream_id}"));
    }
    if let Some(status) = event.metadata.get("grpc.status") {
        let meaning = grpc_status_meaning(status);
        parts.push(format!("gRPC status: {status} ({meaning})"));
    }
    if let Some(msg) = event.metadata.get("grpc.message") {
        parts.push(format!("gRPC message: {msg}"));
    }
    if let Some(authority) = event.metadata.get("grpc.authority") {
        parts.push(format!("Authority: {authority}"));
    }
    if let Some(encoding) = event.metadata.get("grpc.encoding") {
        parts.push(format!("Compression: {encoding}"));
    }
    match event.direction {
        Direction::Outbound => parts.push("Role: Client sending request".into()),
        Direction::Inbound => parts.push("Role: Server response".into()),
        Direction::Unknown => {}
    }
}

fn summarize_zmq(event: &DebugEvent, parts: &mut Vec<String>) {
    if let Some(topic) = event.metadata.get(METADATA_KEY_ZMQ_TOPIC) {
        parts.push(format!("ZMQ topic: {topic}"));
    }
    if let Some(socket_type) = event.metadata.get("zmq.socket_type") {
        parts.push(format!("Socket type: {socket_type}"));
    }
    if let Some(identity) = event.metadata.get("zmq.identity") {
        parts.push(format!("Identity: {identity}"));
    }
}

fn summarize_dds(event: &DebugEvent, parts: &mut Vec<String>) {
    if let Some(domain) = event.metadata.get(METADATA_KEY_DDS_DOMAIN_ID) {
        parts.push(format!("DDS domain: {domain}"));
    }
    if let Some(topic) = event.metadata.get(METADATA_KEY_DDS_TOPIC_NAME) {
        parts.push(format!("DDS topic: {topic}"));
    }
    if let Some(guid) = event.metadata.get("dds.writer_guid") {
        parts.push(format!("Writer GUID: {guid}"));
    }
}

fn summarize_generic(event: &DebugEvent, parts: &mut Vec<String>) {
    if let Some(tls) = event.metadata.get("pcap.tls_decrypted") {
        parts.push(format!("TLS decrypted: {tls}"));
    }
}

fn is_already_summarized(key: &str) -> bool {
    matches!(
        key,
        "grpc.method"
            | "h2.stream_id"
            | "grpc.status"
            | "grpc.message"
            | "grpc.authority"
            | "grpc.encoding"
            | "zmq.topic"
            | "zmq.socket_type"
            | "zmq.identity"
            | "dds.domain_id"
            | "dds.topic_name"
            | "dds.writer_guid"
            | "pcap.tls_decrypted"
    )
}

fn format_timestamp(nanos: u64) -> String {
    let secs = nanos / 1_000_000_000;
    let sub_secs = nanos % 1_000_000_000;
    let millis = sub_secs / 1_000_000;
    format!("{secs}.{millis:03}s (epoch)")
}

fn grpc_status_meaning(code: &str) -> &'static str {
    match code {
        "0" => "OK",
        "1" => "CANCELLED",
        "2" => "UNKNOWN",
        "3" => "INVALID_ARGUMENT",
        "4" => "DEADLINE_EXCEEDED",
        "5" => "NOT_FOUND",
        "6" => "ALREADY_EXISTS",
        "7" => "PERMISSION_DENIED",
        "8" => "RESOURCE_EXHAUSTED",
        "9" => "FAILED_PRECONDITION",
        "10" => "ABORTED",
        "11" => "OUT_OF_RANGE",
        "12" => "UNIMPLEMENTED",
        "13" => "INTERNAL",
        "14" => "UNAVAILABLE",
        "15" => "DATA_LOSS",
        "16" => "UNAUTHENTICATED",
        _ => "UNKNOWN_CODE",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use prb_core::{DebugEvent, EventSource, Payload, Timestamp};

    fn make_grpc_event(id: u64, method: &str, status: &str) -> DebugEvent {
        let mut builder = DebugEvent::builder()
            .id(prb_core::EventId::from_raw(id))
            .timestamp(Timestamp::from_nanos(
                1_710_000_000_000_000_000 + id * 1_000_000,
            ))
            .source(EventSource {
                adapter: "pcap".into(),
                origin: "test.pcap".into(),
                network: Some(prb_core::NetworkAddr {
                    src: "10.0.0.1:52341".into(),
                    dst: "10.0.0.2:50051".into(),
                }),
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Inbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"test payload"),
            })
            .metadata("grpc.method", method)
            .metadata("h2.stream_id", "1");

        if !status.is_empty() {
            builder = builder.metadata("grpc.status", status);
        }

        builder.build()
    }

    fn make_zmq_event(id: u64, topic: &str) -> DebugEvent {
        DebugEvent::builder()
            .id(prb_core::EventId::from_raw(id))
            .timestamp(Timestamp::from_nanos(
                1_710_000_000_000_000_000 + id * 1_000_000,
            ))
            .source(EventSource {
                adapter: "pcap".into(),
                origin: "test.pcap".into(),
                network: None,
            })
            .transport(TransportKind::Zmq)
            .direction(Direction::Unknown)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"zmq data"),
            })
            .metadata("zmq.topic", topic)
            .metadata("zmq.socket_type", "PUB")
            .build()
    }

    fn make_dds_event(id: u64, topic: &str, domain: &str) -> DebugEvent {
        DebugEvent::builder()
            .id(prb_core::EventId::from_raw(id))
            .timestamp(Timestamp::from_nanos(
                1_710_000_000_000_000_000 + id * 1_000_000,
            ))
            .source(EventSource {
                adapter: "pcap".into(),
                origin: "test.pcap".into(),
                network: None,
            })
            .transport(TransportKind::DdsRtps)
            .direction(Direction::Inbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"rtps data"),
            })
            .metadata("dds.topic_name", topic)
            .metadata("dds.domain_id", domain)
            .build()
    }

    #[test]
    fn test_context_builder_grpc() {
        let events = vec![
            make_grpc_event(1, "/api.v1.Users/Get", ""),
            make_grpc_event(2, "/api.v1.Users/Get", "14"),
            make_grpc_event(3, "/api.v1.Users/List", "0"),
        ];
        let ctx = ExplainContext::build(&events, 1, 5);
        assert_eq!(ctx.transport, TransportKind::Grpc);
        assert!(ctx.target_summary.contains("/api.v1.Users/Get"));
        assert!(ctx.target_summary.contains("UNAVAILABLE"));
        assert!(ctx.has_errors);
        assert_eq!(ctx.surrounding_summaries.len(), 2);
    }

    #[test]
    fn test_context_builder_zmq() {
        let events = vec![
            make_zmq_event(1, "market.data"),
            make_zmq_event(2, "market.orders"),
        ];
        let ctx = ExplainContext::build(&events, 0, 5);
        assert_eq!(ctx.transport, TransportKind::Zmq);
        assert!(ctx.target_summary.contains("market.data"));
        assert!(ctx.target_summary.contains("PUB"));
    }

    #[test]
    fn test_context_builder_dds() {
        let events = vec![make_dds_event(1, "rt/chatter", "0")];
        let ctx = ExplainContext::build(&events, 0, 5);
        assert_eq!(ctx.transport, TransportKind::DdsRtps);
        assert!(ctx.target_summary.contains("rt/chatter"));
        assert!(ctx.target_summary.contains("DDS domain: 0"));
    }

    #[test]
    fn test_context_window_selection() {
        let events: Vec<DebugEvent> = (0..20).map(|i| make_grpc_event(i, "/test", "0")).collect();

        let ctx = ExplainContext::build(&events, 10, 3);
        assert_eq!(ctx.surrounding_summaries.len(), 6);

        let ctx_start = ExplainContext::build(&events, 0, 3);
        assert_eq!(ctx_start.surrounding_summaries.len(), 3);

        let ctx_end = ExplainContext::build(&events, 19, 3);
        assert_eq!(ctx_end.surrounding_summaries.len(), 3);
    }

    #[test]
    fn test_find_event_index() {
        let events = vec![
            make_grpc_event(10, "/a", "0"),
            make_grpc_event(20, "/b", "0"),
            make_grpc_event(30, "/c", "0"),
        ];
        assert_eq!(ExplainContext::find_event_index(&events, 20), Some(1));
        assert_eq!(ExplainContext::find_event_index(&events, 99), None);
    }

    #[test]
    fn test_grpc_status_meaning() {
        assert_eq!(grpc_status_meaning("0"), "OK");
        assert_eq!(grpc_status_meaning("14"), "UNAVAILABLE");
        assert_eq!(grpc_status_meaning("16"), "UNAUTHENTICATED");
        assert_eq!(grpc_status_meaning("99"), "UNKNOWN_CODE");
    }
}

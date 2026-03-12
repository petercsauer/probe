//! OpenTelemetry trace context parsing.
//!
//! Supports W3C traceparent, B3 (single and multi-header), and Jaeger uber-trace-id formats.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// W3C Trace Context extracted from protocol headers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceContext {
    /// Trace ID (32 lowercase hex characters).
    pub trace_id: String,
    /// Span ID (16 lowercase hex characters).
    pub span_id: String,
    /// Trace flags (0x01 = sampled).
    pub trace_flags: u8,
    /// Vendor-specific tracestate key=value pairs.
    pub tracestate: Option<String>,
}

impl TraceContext {
    /// Check if the trace is sampled (trace_flags bit 0 set).
    pub fn is_sampled(&self) -> bool {
        self.trace_flags & 0x01 != 0
    }
}

/// Parse W3C traceparent header.
///
/// Format: `00-{trace_id}-{span_id}-{flags}`
/// Example: `00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01`
///
/// Spec: <https://www.w3.org/TR/trace-context/>
pub fn parse_w3c_traceparent(header: &str) -> Option<TraceContext> {
    let parts: Vec<&str> = header.split('-').collect();
    if parts.len() != 4 {
        return None;
    }

    let version = parts[0];
    let trace_id = parts[1];
    let span_id = parts[2];
    let flags_str = parts[3];

    // Version must be "00"
    if version != "00" {
        return None;
    }

    // trace_id: 32 hex chars (16 bytes), not all zeros
    if trace_id.len() != 32 || !trace_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    if trace_id.chars().all(|c| c == '0') {
        return None; // all-zero trace_id is invalid
    }
    // Must be lowercase per spec
    if trace_id.chars().any(|c| c.is_ascii_uppercase()) {
        return None;
    }

    // span_id: 16 hex chars (8 bytes), not all zeros
    if span_id.len() != 16 || !span_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    if span_id.chars().all(|c| c == '0') {
        return None; // all-zero span_id is invalid
    }
    if span_id.chars().any(|c| c.is_ascii_uppercase()) {
        return None;
    }

    // flags: 2 hex chars (1 byte)
    if flags_str.len() != 2 || !flags_str.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let trace_flags = u8::from_str_radix(flags_str, 16).ok()?;

    Some(TraceContext {
        trace_id: trace_id.to_string(),
        span_id: span_id.to_string(),
        trace_flags,
        tracestate: None,
    })
}

/// Parse B3 single-header format.
///
/// Format: `{trace_id}-{span_id}[-{sampling}[-{parent_span_id}]]`
/// Example: `4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-1`
///
/// Spec: <https://github.com/openzipkin/b3-propagation>
pub fn parse_b3_single(header: &str) -> Option<TraceContext> {
    let parts: Vec<&str> = header.split('-').collect();
    if parts.len() < 2 || parts.len() > 4 {
        return None;
    }

    let trace_id = parts[0];
    let span_id = parts[1];
    let sampling = parts.get(2).copied();

    // B3 trace_id can be 16 or 32 hex chars
    if (trace_id.len() != 16 && trace_id.len() != 32)
        || !trace_id.chars().all(|c| c.is_ascii_hexdigit())
    {
        return None;
    }
    // Pad 64-bit trace IDs to 128-bit
    let trace_id_normalized = if trace_id.len() == 16 {
        format!("{:0>32}", trace_id)
    } else {
        trace_id.to_string()
    };

    // span_id: 16 hex chars
    if span_id.len() != 16 || !span_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    // sampling: "0" (deny), "1" (accept), "d" (debug)
    let trace_flags = match sampling {
        Some("1" | "d") => 0x01,
        _ => 0x00,
    };

    Some(TraceContext {
        trace_id: trace_id_normalized.to_lowercase(),
        span_id: span_id.to_lowercase(),
        trace_flags,
        tracestate: None,
    })
}

/// Parse B3 multi-header format.
///
/// Headers: `X-B3-TraceId`, `X-B3-SpanId`, `X-B3-Sampled`, `X-B3-ParentSpanId`
///
/// Spec: <https://github.com/openzipkin/b3-propagation>
pub fn parse_b3_multi(headers: &HashMap<String, String>) -> Option<TraceContext> {
    let trace_id = headers
        .get("x-b3-traceid")
        .or_else(|| headers.get("X-B3-TraceId"))?;
    let span_id = headers
        .get("x-b3-spanid")
        .or_else(|| headers.get("X-B3-SpanId"))?;

    // Same validation as B3 single-header
    if (trace_id.len() != 16 && trace_id.len() != 32)
        || !trace_id.chars().all(|c| c.is_ascii_hexdigit())
    {
        return None;
    }
    let trace_id_normalized = if trace_id.len() == 16 {
        format!("{:0>32}", trace_id)
    } else {
        trace_id.to_string()
    };

    if span_id.len() != 16 || !span_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let sampled = headers
        .get("x-b3-sampled")
        .or_else(|| headers.get("X-B3-Sampled"));
    let trace_flags = match sampled.map(|s| s.as_str()) {
        Some("1" | "d") => 0x01,
        _ => 0x00,
    };

    Some(TraceContext {
        trace_id: trace_id_normalized.to_lowercase(),
        span_id: span_id.to_lowercase(),
        trace_flags,
        tracestate: None,
    })
}

/// Parse Jaeger uber-trace-id header.
///
/// Format: `{trace_id}:{span_id}:{parent_span_id}:{flags}`
/// Example: `4bf92f3577b34da6a3ce929d0e0e4736:00f067aa0ba902b7:0:1`
///
/// Spec: <https://www.jaegertracing.io/docs/1.21/client-libraries/#propagation-format>
pub fn parse_uber_trace_id(header: &str) -> Option<TraceContext> {
    let parts: Vec<&str> = header.split(':').collect();
    if parts.len() != 4 {
        return None;
    }

    let trace_id = parts[0];
    let span_id = parts[1];
    let flags_str = parts[3];

    // Jaeger trace_id: 16 or 32 hex chars
    if (trace_id.len() != 16 && trace_id.len() != 32)
        || !trace_id.chars().all(|c| c.is_ascii_hexdigit())
    {
        return None;
    }
    let trace_id_normalized = if trace_id.len() == 16 {
        format!("{:0>32}", trace_id)
    } else {
        trace_id.to_string()
    };

    // span_id: 16 hex chars
    if span_id.len() != 16 || !span_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    // flags: integer, bit 0 = sampled
    let flags: u8 = flags_str.parse().ok()?;
    let trace_flags = if flags & 0x01 != 0 { 0x01 } else { 0x00 };

    Some(TraceContext {
        trace_id: trace_id_normalized.to_lowercase(),
        span_id: span_id.to_lowercase(),
        trace_flags,
        tracestate: None,
    })
}

/// Extract trace context from headers, trying all known formats.
///
/// Priority order: W3C traceparent > B3 single > B3 multi > uber-trace-id
pub fn extract_trace_context(headers: &HashMap<String, String>) -> Option<TraceContext> {
    // Try W3C traceparent first (standard)
    if let Some(traceparent) = headers.get("traceparent")
        && let Some(ctx) = parse_w3c_traceparent(traceparent)
    {
        // Also read tracestate if present
        let mut ctx = ctx;
        if let Some(tracestate) = headers.get("tracestate") {
            ctx.tracestate = Some(tracestate.clone());
        }
        return Some(ctx);
    }

    // Try B3 single-header
    if let Some(b3) = headers.get("b3")
        && let Some(ctx) = parse_b3_single(b3)
    {
        return Some(ctx);
    }

    // Try B3 multi-header
    if (headers.contains_key("x-b3-traceid") || headers.contains_key("X-B3-TraceId"))
        && let Some(ctx) = parse_b3_multi(headers)
    {
        return Some(ctx);
    }

    // Try Jaeger uber-trace-id
    if let Some(uber) = headers.get("uber-trace-id")
        && let Some(ctx) = parse_uber_trace_id(uber)
    {
        return Some(ctx);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_w3c_traceparent_valid() {
        let header = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let ctx = parse_w3c_traceparent(header).unwrap();
        assert_eq!(ctx.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(ctx.span_id, "00f067aa0ba902b7");
        assert_eq!(ctx.trace_flags, 0x01);
        assert!(ctx.is_sampled());
    }

    #[test]
    fn test_parse_w3c_traceparent_unsampled() {
        let header = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00";
        let ctx = parse_w3c_traceparent(header).unwrap();
        assert_eq!(ctx.trace_flags, 0x00);
        assert!(!ctx.is_sampled());
    }

    #[test]
    fn test_parse_w3c_traceparent_invalid_version() {
        let header = "01-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        assert!(parse_w3c_traceparent(header).is_none());
    }

    #[test]
    fn test_parse_w3c_traceparent_all_zero_trace_id() {
        let header = "00-00000000000000000000000000000000-00f067aa0ba902b7-01";
        assert!(parse_w3c_traceparent(header).is_none());
    }

    #[test]
    fn test_parse_w3c_traceparent_all_zero_span_id() {
        let header = "00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01";
        assert!(parse_w3c_traceparent(header).is_none());
    }

    #[test]
    fn test_parse_w3c_traceparent_uppercase() {
        let header = "00-4BF92F3577B34DA6A3CE929D0E0E4736-00F067AA0BA902B7-01";
        assert!(parse_w3c_traceparent(header).is_none());
    }

    #[test]
    fn test_parse_w3c_traceparent_wrong_length() {
        let header = "00-4bf92f3577b34da6-00f067aa0ba902b7-01";
        assert!(parse_w3c_traceparent(header).is_none());
    }

    #[test]
    fn test_parse_b3_single_full() {
        let header = "4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-1-parent123";
        let ctx = parse_b3_single(header).unwrap();
        assert_eq!(ctx.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(ctx.span_id, "00f067aa0ba902b7");
        assert_eq!(ctx.trace_flags, 0x01);
    }

    #[test]
    fn test_parse_b3_single_minimal() {
        let header = "4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7";
        let ctx = parse_b3_single(header).unwrap();
        assert_eq!(ctx.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(ctx.span_id, "00f067aa0ba902b7");
        assert_eq!(ctx.trace_flags, 0x00);
    }

    #[test]
    fn test_parse_b3_single_64bit_trace_id() {
        let header = "a3ce929d0e0e4736-00f067aa0ba902b7-1";
        let ctx = parse_b3_single(header).unwrap();
        // Should be padded to 32 hex chars
        assert_eq!(ctx.trace_id, "0000000000000000a3ce929d0e0e4736");
        assert_eq!(ctx.span_id, "00f067aa0ba902b7");
    }

    #[test]
    fn test_parse_b3_multi_headers() {
        let mut headers = HashMap::new();
        headers.insert(
            "X-B3-TraceId".to_string(),
            "4bf92f3577b34da6a3ce929d0e0e4736".to_string(),
        );
        headers.insert("X-B3-SpanId".to_string(), "00f067aa0ba902b7".to_string());
        headers.insert("X-B3-Sampled".to_string(), "1".to_string());

        let ctx = parse_b3_multi(&headers).unwrap();
        assert_eq!(ctx.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(ctx.span_id, "00f067aa0ba902b7");
        assert_eq!(ctx.trace_flags, 0x01);
    }

    #[test]
    fn test_parse_b3_multi_lowercase_headers() {
        let mut headers = HashMap::new();
        headers.insert(
            "x-b3-traceid".to_string(),
            "4bf92f3577b34da6a3ce929d0e0e4736".to_string(),
        );
        headers.insert("x-b3-spanid".to_string(), "00f067aa0ba902b7".to_string());

        let ctx = parse_b3_multi(&headers).unwrap();
        assert_eq!(ctx.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(ctx.span_id, "00f067aa0ba902b7");
    }

    #[test]
    fn test_parse_uber_trace_id() {
        let header = "4bf92f3577b34da6a3ce929d0e0e4736:00f067aa0ba902b7:0:1";
        let ctx = parse_uber_trace_id(header).unwrap();
        assert_eq!(ctx.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(ctx.span_id, "00f067aa0ba902b7");
        assert_eq!(ctx.trace_flags, 0x01);
    }

    #[test]
    fn test_extract_trace_context_priority() {
        let mut headers = HashMap::new();
        headers.insert(
            "traceparent".to_string(),
            "00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01".to_string(),
        );
        headers.insert(
            "b3".to_string(),
            "cccccccccccccccccccccccccccccccc-dddddddddddddddd-1".to_string(),
        );

        let ctx = extract_trace_context(&headers).unwrap();
        // W3C should win
        assert_eq!(ctx.trace_id, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(ctx.span_id, "bbbbbbbbbbbbbbbb");
    }

    #[test]
    fn test_trace_context_serde_roundtrip() {
        let ctx = TraceContext {
            trace_id: "4bf92f3577b34da6a3ce929d0e0e4736".to_string(),
            span_id: "00f067aa0ba902b7".to_string(),
            trace_flags: 0x01,
            tracestate: Some("vendor=value".to_string()),
        };

        let json = serde_json::to_string(&ctx).unwrap();
        let ctx2: TraceContext = serde_json::from_str(&json).unwrap();
        assert_eq!(ctx, ctx2);
    }
}

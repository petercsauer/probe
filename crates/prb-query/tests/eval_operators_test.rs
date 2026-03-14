use bytes::Bytes;
use prb_core::{
    CorrelationKey, DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp,
    TransportKind,
};
use prb_query::eval::eval;
use prb_query::parser::parse_expr;
use std::collections::BTreeMap;

fn sample_event() -> DebugEvent {
    let mut metadata = BTreeMap::new();
    metadata.insert("grpc.method".to_string(), "/api.v1.Users/Get".to_string());
    metadata.insert("grpc.status".to_string(), "0".to_string());
    metadata.insert("http.host".to_string(), "Example.Com".to_string());
    metadata.insert("tcp.payload".to_string(), "GET /api HTTP/1.1".to_string());

    DebugEvent {
        id: EventId::from_raw(42),
        timestamp: Timestamp::from_nanos(1_710_000_000_000_000_000),
        source: EventSource {
            adapter: "pcap".to_string(),
            origin: "capture.pcap".to_string(),
            network: Some(NetworkAddr {
                src: "10.0.0.1:42837".to_string(),
                dst: "10.0.0.2:50051".to_string(),
            }),
        },
        transport: TransportKind::Grpc,
        direction: Direction::Outbound,
        payload: Payload::Raw {
            raw: Bytes::from_static(b"hello world"),
        },
        metadata,
        correlation_keys: vec![CorrelationKey::StreamId { id: 1 }],
        sequence: Some(1),
        warnings: vec![],
    }
}

fn event_with_tcp_port(port: u16) -> DebugEvent {
    let mut metadata = BTreeMap::new();
    metadata.insert("tcp.port".to_string(), port.to_string());
    metadata.insert("tcp.payload".to_string(), "GET / HTTP/1.1".to_string());

    DebugEvent {
        id: EventId::from_raw(1),
        timestamp: Timestamp::from_nanos(1_000_000_000),
        source: EventSource {
            adapter: "test".to_string(),
            origin: "test".to_string(),
            network: None,
        },
        transport: TransportKind::RawTcp,
        direction: Direction::Inbound,
        payload: Payload::Raw { raw: Bytes::new() },
        metadata,
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}

#[test]
fn test_eval_matches_regex() {
    let event = sample_event();
    let expr = parse_expr(r#"tcp.payload matches "^GET""#).unwrap();
    assert!(eval(&expr, &event));
}

#[test]
fn test_eval_matches_regex_no_match() {
    let event = sample_event();
    let expr = parse_expr(r#"tcp.payload matches "^POST""#).unwrap();
    assert!(!eval(&expr, &event));
}

#[test]
fn test_eval_matches_complex_pattern() {
    let event = sample_event();
    let expr = parse_expr(r#"tcp.payload matches "GET.*/api.*HTTP""#).unwrap();
    assert!(eval(&expr, &event));
}

#[test]
fn test_eval_matches_invalid_regex() {
    let event = sample_event();
    let expr = parse_expr(r#"tcp.payload matches "[invalid""#).unwrap();
    assert!(!eval(&expr, &event));
}

#[test]
fn test_eval_matches_missing_field() {
    let event = sample_event();
    let expr = parse_expr(r#"nonexistent matches "pattern""#).unwrap();
    assert!(!eval(&expr, &event));
}

#[test]
fn test_eval_in_numbers() {
    let event80 = event_with_tcp_port(80);
    let event443 = event_with_tcp_port(443);
    let event22 = event_with_tcp_port(22);

    let expr = parse_expr("tcp.port in {80, 443, 8080}").unwrap();
    assert!(eval(&expr, &event80));
    assert!(eval(&expr, &event443));
    assert!(!eval(&expr, &event22));
}

#[test]
fn test_eval_in_strings() {
    let event = sample_event();
    let expr = parse_expr(r#"transport in {"gRPC", "HTTP", "ZMQ"}"#).unwrap();
    assert!(eval(&expr, &event));
}

#[test]
fn test_eval_in_strings_no_match() {
    let event = sample_event();
    let expr = parse_expr(r#"transport in {"TCP", "UDP"}"#).unwrap();
    assert!(!eval(&expr, &event));
}

#[test]
fn test_eval_in_bools() {
    let mut event = sample_event();
    event
        .metadata
        .insert("flag".to_string(), "true".to_string());

    let expr = parse_expr("flag in {true, false}").unwrap();
    assert!(eval(&expr, &event));
}

#[test]
fn test_eval_in_missing_field() {
    let event = sample_event();
    let expr = parse_expr("nonexistent in {1, 2, 3}").unwrap();
    assert!(!eval(&expr, &event));
}

#[test]
fn test_eval_in_single_value() {
    let event = event_with_tcp_port(80);
    let expr = parse_expr("tcp.port in {80}").unwrap();
    assert!(eval(&expr, &event));
}

#[test]
fn test_eval_slice_valid_range() {
    let mut event = sample_event();
    event
        .metadata
        .insert("data".to_string(), "hello world".to_string());

    let expr = parse_expr("data[0:5]").unwrap();
    assert!(eval(&expr, &event));
}

#[test]
fn test_eval_slice_out_of_bounds() {
    let mut event = sample_event();
    event
        .metadata
        .insert("data".to_string(), "short".to_string());

    let expr = parse_expr("data[0:100]").unwrap();
    assert!(!eval(&expr, &event));
}

#[test]
fn test_eval_slice_missing_field() {
    let event = sample_event();
    let expr = parse_expr("nonexistent[0:5]").unwrap();
    assert!(!eval(&expr, &event));
}

#[test]
fn test_eval_function_len() {
    let mut event = sample_event();
    event
        .metadata
        .insert("payload".to_string(), "test data".to_string());

    let expr = parse_expr("len(payload)").unwrap();
    assert!(eval(&expr, &event));
}

#[test]
fn test_eval_function_len_missing_field() {
    let event = sample_event();
    let expr = parse_expr("len(nonexistent)").unwrap();
    assert!(!eval(&expr, &event));
}

#[test]
fn test_eval_function_lower() {
    let event = sample_event();
    let expr = parse_expr("lower(http.host)").unwrap();
    assert!(eval(&expr, &event));
}

#[test]
fn test_eval_function_upper() {
    let event = sample_event();
    let expr = parse_expr("upper(http.host)").unwrap();
    assert!(eval(&expr, &event));
}

#[test]
fn test_eval_function_unknown() {
    let event = sample_event();
    let expr = parse_expr("unknown(field)").unwrap();
    assert!(!eval(&expr, &event));
}

#[test]
fn test_eval_function_wrong_arg_count() {
    let event = sample_event();
    let expr = parse_expr("len(field1, field2)").unwrap();
    assert!(!eval(&expr, &event));
}

#[test]
fn test_eval_complex_filter() {
    let event = event_with_tcp_port(80);
    let expr = parse_expr(r#"tcp.port in {80, 443} && tcp.payload matches "^GET""#).unwrap();
    assert!(eval(&expr, &event));
}

#[test]
fn test_eval_complex_filter_partial_match() {
    let event = event_with_tcp_port(22);
    let expr = parse_expr(r#"tcp.port in {80, 443} && tcp.payload matches "^GET""#).unwrap();
    assert!(!eval(&expr, &event));
}

#[test]
fn test_eval_or_with_matches() {
    let event = sample_event();
    let expr = parse_expr(r#"tcp.payload matches "POST" || tcp.payload matches "GET""#).unwrap();
    assert!(eval(&expr, &event));
}

#[test]
fn test_eval_not_with_in() {
    let event = event_with_tcp_port(80);
    let expr = parse_expr("!tcp.port in {22, 23, 3389}").unwrap();
    assert!(eval(&expr, &event));
}

#[test]
fn test_eval_not_with_matches() {
    let event = sample_event();
    let expr = parse_expr(r#"!tcp.payload matches "^POST""#).unwrap();
    assert!(eval(&expr, &event));
}

#[test]
fn test_eval_nested_functions() {
    let event = sample_event();
    let expr = parse_expr("lower(upper(http.host))").unwrap();
    assert!(eval(&expr, &event));
}

#[test]
fn test_eval_parentheses_with_new_operators() {
    let event = event_with_tcp_port(80);
    let expr = parse_expr(
        r#"(tcp.payload matches "GET" || tcp.payload matches "POST") && tcp.port in {80, 443}"#,
    )
    .unwrap();
    assert!(eval(&expr, &event));
}

#[test]
fn test_eval_matches_case_sensitive() {
    let event = sample_event();
    let expr = parse_expr(r#"tcp.payload matches "^get""#).unwrap();
    assert!(!eval(&expr, &event)); // GET != get
}

#[test]
fn test_eval_matches_case_insensitive_pattern() {
    let event = sample_event();
    let expr = parse_expr(r#"tcp.payload matches "(?i)^get""#).unwrap();
    assert!(eval(&expr, &event)); // Case-insensitive flag
}

#[test]
fn test_eval_in_with_mixed_types() {
    let mut event = sample_event();
    event.metadata.insert("value".to_string(), "42".to_string());

    let expr = parse_expr(r#"value in {42, "42", true}"#).unwrap();
    assert!(eval(&expr, &event)); // Matches as number
}

#[test]
fn test_eval_slice_at_exact_boundary() {
    let mut event = sample_event();
    event
        .metadata
        .insert("data".to_string(), "12345".to_string());

    let expr = parse_expr("data[0:5]").unwrap();
    assert!(eval(&expr, &event)); // Exactly matches length
}

#[test]
fn test_eval_slice_one_past_end() {
    let mut event = sample_event();
    event
        .metadata
        .insert("data".to_string(), "12345".to_string());

    let expr = parse_expr("data[0:6]").unwrap();
    assert!(!eval(&expr, &event)); // One byte too many
}

#[test]
fn test_eval_function_with_no_args() {
    let event = sample_event();
    let expr = parse_expr("now()").unwrap();
    assert!(!eval(&expr, &event)); // Unknown function returns false
}

#[test]
fn test_eval_complex_and_or_precedence() {
    let event = event_with_tcp_port(80);
    // a || b && c should be parsed as a || (b && c)
    let expr = parse_expr(r#"tcp.port in {22} || tcp.port in {80} && tcp.payload matches "^GET""#)
        .unwrap();
    assert!(eval(&expr, &event)); // Matches second part: 80 is in set AND payload matches
}

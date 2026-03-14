use bytes::Bytes;
use prb_core::{
    DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind,
};
use prb_query::Filter;

fn tcp_event(src: &str, dst: &str) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(1),
        timestamp: Timestamp::from_nanos(1_000_000_000),
        source: EventSource {
            adapter: "test".to_string(),
            origin: "test".to_string(),
            network: Some(NetworkAddr {
                src: src.to_string(),
                dst: dst.to_string(),
            }),
        },
        transport: TransportKind::RawTcp,
        direction: Direction::Outbound,
        payload: Payload::Raw { raw: Bytes::new() },
        metadata: Default::default(),
        correlation_keys: vec![],
        sequence: Some(1),
        warnings: vec![],
    }
}

fn udp_event(src: &str, dst: &str) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(2),
        timestamp: Timestamp::from_nanos(1_000_000_000),
        source: EventSource {
            adapter: "test".to_string(),
            origin: "test".to_string(),
            network: Some(NetworkAddr {
                src: src.to_string(),
                dst: dst.to_string(),
            }),
        },
        transport: TransportKind::RawUdp,
        direction: Direction::Outbound,
        payload: Payload::Raw { raw: Bytes::new() },
        metadata: Default::default(),
        correlation_keys: vec![],
        sequence: Some(2),
        warnings: vec![],
    }
}

#[test]
fn test_tcp_port_extraction_ipv4() {
    let event = tcp_event("192.168.1.1:12345", "10.0.0.1:443");

    // tcp.port should match either src or dst port
    let filter = Filter::parse("tcp.port == 443").unwrap();
    assert!(filter.matches(&event));

    let filter = Filter::parse("tcp.port == 12345").unwrap();
    assert!(filter.matches(&event));

    let filter = Filter::parse("tcp.port == 80").unwrap();
    assert!(!filter.matches(&event));
}

#[test]
fn test_tcp_port_extraction_ipv6() {
    let event = tcp_event("[::1]:8080", "[fe80::1]:443");

    let filter = Filter::parse("tcp.port == 8080").unwrap();
    assert!(filter.matches(&event));

    let filter = Filter::parse("tcp.port == 443").unwrap();
    assert!(filter.matches(&event));
}

#[test]
fn test_tcp_srcport_dstport() {
    let event = tcp_event("192.168.1.1:12345", "10.0.0.1:443");

    let filter = Filter::parse("tcp.srcport == 12345").unwrap();
    assert!(filter.matches(&event));

    let filter = Filter::parse("tcp.dstport == 443").unwrap();
    assert!(filter.matches(&event));

    let filter = Filter::parse("tcp.srcport == 443").unwrap();
    assert!(!filter.matches(&event));
}

#[test]
fn test_udp_port_extraction() {
    let event = udp_event("192.168.1.1:53", "10.0.0.1:12345");

    let filter = Filter::parse("udp.port == 53").unwrap();
    assert!(filter.matches(&event));

    let filter = Filter::parse("udp.port == 12345").unwrap();
    assert!(filter.matches(&event));

    let filter = Filter::parse("udp.port == 5353").unwrap();
    assert!(!filter.matches(&event));
}

#[test]
fn test_udp_srcport_dstport() {
    let event = udp_event("192.168.1.1:53", "10.0.0.1:12345");

    let filter = Filter::parse("udp.srcport == 53").unwrap();
    assert!(filter.matches(&event));

    let filter = Filter::parse("udp.dstport == 12345").unwrap();
    assert!(filter.matches(&event));
}

#[test]
fn test_protocol_mismatch_tcp_on_udp() {
    let event = udp_event("192.168.1.1:53", "10.0.0.1:12345");

    // tcp.port on UDP event should not match
    let filter = Filter::parse("tcp.port == 53").unwrap();
    assert!(!filter.matches(&event));
}

#[test]
fn test_protocol_mismatch_udp_on_tcp() {
    let event = tcp_event("192.168.1.1:443", "10.0.0.1:12345");

    // udp.port on TCP event should not match
    let filter = Filter::parse("udp.port == 443").unwrap();
    assert!(!filter.matches(&event));
}

#[test]
fn test_ip_src_extraction() {
    let event = tcp_event("192.168.1.1:12345", "10.0.0.1:443");

    let filter = Filter::parse(r#"ip.src == "192.168.1.1""#).unwrap();
    assert!(filter.matches(&event));

    let filter = Filter::parse(r#"ip.src == "10.0.0.1""#).unwrap();
    assert!(!filter.matches(&event));
}

#[test]
fn test_ip_dst_extraction() {
    let event = tcp_event("192.168.1.1:12345", "10.0.0.1:443");

    let filter = Filter::parse(r#"ip.dst == "10.0.0.1""#).unwrap();
    assert!(filter.matches(&event));

    let filter = Filter::parse(r#"ip.dst == "192.168.1.1""#).unwrap();
    assert!(!filter.matches(&event));
}

#[test]
fn test_ip_addr_matches_either() {
    let event = tcp_event("192.168.1.1:12345", "10.0.0.1:443");

    // ip.addr should match either src or dst
    let filter = Filter::parse(r#"ip.addr == "192.168.1.1""#).unwrap();
    assert!(filter.matches(&event));

    let filter = Filter::parse(r#"ip.addr == "10.0.0.1""#).unwrap();
    assert!(filter.matches(&event));

    let filter = Filter::parse(r#"ip.addr == "172.16.0.1""#).unwrap();
    assert!(!filter.matches(&event));
}

#[test]
fn test_ip_extraction_ipv6() {
    let event = tcp_event("[::1]:8080", "[fe80::1]:443");

    let filter = Filter::parse(r#"ip.src == "::1""#).unwrap();
    assert!(filter.matches(&event));

    let filter = Filter::parse(r#"ip.dst == "fe80::1""#).unwrap();
    assert!(filter.matches(&event));
}

#[test]
fn test_frame_len_field() {
    let event = tcp_event("192.168.1.1:12345", "10.0.0.1:443");

    // frame.len should exist (using frame number as proxy)
    let filter = Filter::parse("frame.len == 1").unwrap();
    assert!(filter.matches(&event));
}

#[test]
fn test_critical_bug_udp_port_5353() {
    // This is the critical bug fix: udp.port==5353 should ONLY match UDP traffic
    let udp_event = udp_event("192.168.1.1:5353", "10.0.0.1:12345");
    let tcp_event = tcp_event("192.168.1.1:5353", "10.0.0.1:443");

    let filter = Filter::parse("udp.port == 5353").unwrap();

    // Should match UDP event
    assert!(filter.matches(&udp_event));

    // Should NOT match TCP event (this was the bug!)
    assert!(!filter.matches(&tcp_event));
}

#[test]
fn test_complex_filter_with_ports() {
    let tcp_event = tcp_event("192.168.1.1:80", "10.0.0.1:12345");
    let udp_event = udp_event("192.168.1.1:53", "10.0.0.1:12345");

    let filter = Filter::parse(r#"tcp.port == 80 && ip.src == "192.168.1.1""#).unwrap();
    assert!(filter.matches(&tcp_event));
    assert!(!filter.matches(&udp_event));
}

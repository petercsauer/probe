//! Built-in demo dataset for exploring prb without real capture data.

use bytes::Bytes;
use prb_core::{
    CorrelationKey, DebugEvent, Direction, EventSource, NetworkAddr, Payload, Timestamp,
    TransportKind, METADATA_KEY_GRPC_METHOD, METADATA_KEY_ZMQ_TOPIC, METADATA_KEY_DDS_TOPIC_NAME,
    METADATA_KEY_DDS_DOMAIN_ID, METADATA_KEY_H2_STREAM_ID,
};

/// Generate a synthetic dataset with ~50 events covering all transports.
pub fn generate_demo_events() -> Vec<DebugEvent> {
    let mut events = Vec::new();
    let base_time = 1700000000_000_000_000u64; // Base timestamp in nanos

    // 3 gRPC request/response pairs (one with error status)
    events.push(
        DebugEvent::builder()
            .timestamp(Timestamp::from_nanos(base_time))
            .source(EventSource {
                adapter: "demo".to_string(),
                origin: "synthetic".to_string(),
                network: Some(NetworkAddr {
                    src: "10.0.1.5:45678".to_string(),
                    dst: "10.0.2.10:8080".to_string(),
                }),
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"\x00\x00\x00\x0a\x0a\x08user_123"),
            })
            .metadata(METADATA_KEY_GRPC_METHOD, "/auth.AuthService/Login")
            .metadata(METADATA_KEY_H2_STREAM_ID, "1")
            .correlation_key(CorrelationKey::StreamId { id: 1 })
            .sequence(1)
            .build(),
    );

    events.push(
        DebugEvent::builder()
            .timestamp(Timestamp::from_nanos(base_time + 15_000_000)) // +15ms
            .source(EventSource {
                adapter: "demo".to_string(),
                origin: "synthetic".to_string(),
                network: Some(NetworkAddr {
                    src: "10.0.2.10:8080".to_string(),
                    dst: "10.0.1.5:45678".to_string(),
                }),
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Inbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"\x00\x00\x00\x12\x0a\x10token_abc123xyz"),
            })
            .metadata(METADATA_KEY_GRPC_METHOD, "/auth.AuthService/Login")
            .metadata(METADATA_KEY_H2_STREAM_ID, "1")
            .correlation_key(CorrelationKey::StreamId { id: 1 })
            .sequence(2)
            .build(),
    );

    // Second gRPC call
    events.push(
        DebugEvent::builder()
            .timestamp(Timestamp::from_nanos(base_time + 50_000_000)) // +50ms
            .source(EventSource {
                adapter: "demo".to_string(),
                origin: "synthetic".to_string(),
                network: Some(NetworkAddr {
                    src: "10.0.1.5:45678".to_string(),
                    dst: "10.0.2.10:8080".to_string(),
                }),
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"\x00\x00\x00\x08\x08\x01\x12\x04test"),
            })
            .metadata(METADATA_KEY_GRPC_METHOD, "/data.DataService/GetUser")
            .metadata(METADATA_KEY_H2_STREAM_ID, "3")
            .correlation_key(CorrelationKey::StreamId { id: 3 })
            .sequence(3)
            .build(),
    );

    events.push(
        DebugEvent::builder()
            .timestamp(Timestamp::from_nanos(base_time + 65_000_000)) // +65ms
            .source(EventSource {
                adapter: "demo".to_string(),
                origin: "synthetic".to_string(),
                network: Some(NetworkAddr {
                    src: "10.0.2.10:8080".to_string(),
                    dst: "10.0.1.5:45678".to_string(),
                }),
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Inbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"\x00\x00\x00\x10\x0a\x08John Doe\x12\x04user"),
            })
            .metadata(METADATA_KEY_GRPC_METHOD, "/data.DataService/GetUser")
            .metadata(METADATA_KEY_H2_STREAM_ID, "3")
            .correlation_key(CorrelationKey::StreamId { id: 3 })
            .sequence(4)
            .build(),
    );

    // Third gRPC call with error
    events.push(
        DebugEvent::builder()
            .timestamp(Timestamp::from_nanos(base_time + 100_000_000)) // +100ms
            .source(EventSource {
                adapter: "demo".to_string(),
                origin: "synthetic".to_string(),
                network: Some(NetworkAddr {
                    src: "10.0.1.5:45678".to_string(),
                    dst: "10.0.2.10:8080".to_string(),
                }),
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"\x00\x00\x00\x0c\x0a\x0ainvalid_id"),
            })
            .metadata(METADATA_KEY_GRPC_METHOD, "/data.DataService/DeleteUser")
            .metadata(METADATA_KEY_H2_STREAM_ID, "5")
            .correlation_key(CorrelationKey::StreamId { id: 5 })
            .sequence(5)
            .build(),
    );

    events.push(
        DebugEvent::builder()
            .timestamp(Timestamp::from_nanos(base_time + 120_000_000)) // +120ms
            .source(EventSource {
                adapter: "demo".to_string(),
                origin: "synthetic".to_string(),
                network: Some(NetworkAddr {
                    src: "10.0.2.10:8080".to_string(),
                    dst: "10.0.1.5:45678".to_string(),
                }),
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Inbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"\x00\x00\x00\x08"),
            })
            .metadata(METADATA_KEY_GRPC_METHOD, "/data.DataService/DeleteUser")
            .metadata(METADATA_KEY_H2_STREAM_ID, "5")
            .metadata("grpc.status", "5") // NOT_FOUND
            .correlation_key(CorrelationKey::StreamId { id: 5 })
            .sequence(6)
            .warning("gRPC error status: NOT_FOUND")
            .build(),
    );

    // 2 ZMQ pub/sub flows
    for i in 0..2 {
        let offset = base_time + 150_000_000 + i * 30_000_000;
        events.push(
            DebugEvent::builder()
                .timestamp(Timestamp::from_nanos(offset))
                .source(EventSource {
                    adapter: "demo".to_string(),
                    origin: "synthetic".to_string(),
                    network: Some(NetworkAddr {
                        src: "10.0.3.15:5555".to_string(),
                        dst: "10.0.3.20:5556".to_string(),
                    }),
                })
                .transport(TransportKind::Zmq)
                .direction(Direction::Outbound)
                .payload(Payload::Raw {
                    raw: Bytes::from(format!("sensor.temperature value={}", 20 + i)),
                })
                .metadata(METADATA_KEY_ZMQ_TOPIC, "sensor.temperature")
                .correlation_key(CorrelationKey::Topic {
                    name: "sensor.temperature".to_string(),
                })
                .sequence(7 + i)
                .build(),
        );
    }

    // 1 DDS-RTPS exchange
    events.push(
        DebugEvent::builder()
            .timestamp(Timestamp::from_nanos(base_time + 200_000_000)) // +200ms
            .source(EventSource {
                adapter: "demo".to_string(),
                origin: "synthetic".to_string(),
                network: Some(NetworkAddr {
                    src: "10.0.4.30:7400".to_string(),
                    dst: "239.255.0.1:7400".to_string(),
                }),
            })
            .transport(TransportKind::DdsRtps)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"RTPS\x02\x03\x00\x00\x01\x02\x03\x04"),
            })
            .metadata(METADATA_KEY_DDS_DOMAIN_ID, "0")
            .metadata(METADATA_KEY_DDS_TOPIC_NAME, "RobotState")
            .correlation_key(CorrelationKey::Topic {
                name: "RobotState".to_string(),
            })
            .sequence(9)
            .build(),
    );

    // 2 raw TCP connections
    for i in 0..2 {
        let offset = base_time + 250_000_000 + i * 20_000_000;
        events.push(
            DebugEvent::builder()
                .timestamp(Timestamp::from_nanos(offset))
                .source(EventSource {
                    adapter: "demo".to_string(),
                    origin: "synthetic".to_string(),
                    network: Some(NetworkAddr {
                        src: format!("10.0.5.{}:12345", 40 + i),
                        dst: "10.0.5.50:8000".to_string(),
                    }),
                })
                .transport(TransportKind::RawTcp)
                .direction(Direction::Outbound)
                .payload(Payload::Raw {
                    raw: Bytes::from(format!("GET /api/v1/status HTTP/1.1\r\nHost: server{}\r\n\r\n", i)),
                })
                .correlation_key(CorrelationKey::ConnectionId {
                    id: format!("tcp-{}", i + 1),
                })
                .sequence(10 + i)
                .build(),
        );
    }

    // Fill remaining slots with additional events to reach ~50 total
    // Add more gRPC calls
    for i in 0..15 {
        let offset = base_time + 300_000_000 + i * 15_000_000;
        let direction = if i % 2 == 0 {
            Direction::Outbound
        } else {
            Direction::Inbound
        };
        let (src, dst) = if direction == Direction::Outbound {
            ("10.0.1.5:45678", "10.0.2.10:8080")
        } else {
            ("10.0.2.10:8080", "10.0.1.5:45678")
        };

        events.push(
            DebugEvent::builder()
                .timestamp(Timestamp::from_nanos(offset))
                .source(EventSource {
                    adapter: "demo".to_string(),
                    origin: "synthetic".to_string(),
                    network: Some(NetworkAddr {
                        src: src.to_string(),
                        dst: dst.to_string(),
                    }),
                })
                .transport(TransportKind::Grpc)
                .direction(direction)
                .payload(Payload::Raw {
                    raw: Bytes::from(format!("\x00\x00\x00\x08payload_{}", i)),
                })
                .metadata(METADATA_KEY_GRPC_METHOD, "/test.TestService/Echo")
                .metadata(METADATA_KEY_H2_STREAM_ID, &format!("{}", 7 + i * 2))
                .correlation_key(CorrelationKey::StreamId { id: 7 + i as u32 * 2 })
                .sequence(12 + i)
                .build(),
        );
    }

    // Add more ZMQ messages
    for i in 0..10 {
        let offset = base_time + 600_000_000 + i * 10_000_000;
        events.push(
            DebugEvent::builder()
                .timestamp(Timestamp::from_nanos(offset))
                .source(EventSource {
                    adapter: "demo".to_string(),
                    origin: "synthetic".to_string(),
                    network: Some(NetworkAddr {
                        src: "10.0.3.15:5555".to_string(),
                        dst: "10.0.3.20:5556".to_string(),
                    }),
                })
                .transport(TransportKind::Zmq)
                .direction(Direction::Outbound)
                .payload(Payload::Raw {
                    raw: Bytes::from(format!("metrics.cpu usage={}", 30 + i * 5)),
                })
                .metadata(METADATA_KEY_ZMQ_TOPIC, "metrics.cpu")
                .correlation_key(CorrelationKey::Topic {
                    name: "metrics.cpu".to_string(),
                })
                .sequence(27 + i)
                .build(),
        );
    }

    // Add more DDS messages
    for i in 0..8 {
        let offset = base_time + 800_000_000 + i * 25_000_000;
        events.push(
            DebugEvent::builder()
                .timestamp(Timestamp::from_nanos(offset))
                .source(EventSource {
                    adapter: "demo".to_string(),
                    origin: "synthetic".to_string(),
                    network: Some(NetworkAddr {
                        src: "10.0.4.30:7400".to_string(),
                        dst: "239.255.0.1:7400".to_string(),
                    }),
                })
                .transport(TransportKind::DdsRtps)
                .direction(Direction::Outbound)
                .payload(Payload::Raw {
                    raw: Bytes::from(format!("RTPS\x02\x03\x00\x00data_{}", i)),
                })
                .metadata(METADATA_KEY_DDS_DOMAIN_ID, "0")
                .metadata(METADATA_KEY_DDS_TOPIC_NAME, "SensorData")
                .correlation_key(CorrelationKey::Topic {
                    name: "SensorData".to_string(),
                })
                .sequence(37 + i)
                .build(),
        );
    }

    // Add more TCP connections with warnings
    for i in 0..5 {
        let offset = base_time + 1_100_000_000 + i * 30_000_000;
        let mut builder = DebugEvent::builder()
            .timestamp(Timestamp::from_nanos(offset))
            .source(EventSource {
                adapter: "demo".to_string(),
                origin: "synthetic".to_string(),
                network: Some(NetworkAddr {
                    src: format!("10.0.5.{}:23456", 60 + i),
                    dst: "10.0.5.100:9000".to_string(),
                }),
            })
            .transport(TransportKind::RawTcp)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from(format!("CONNECT server{}.local\r\n", i)),
            })
            .correlation_key(CorrelationKey::ConnectionId {
                id: format!("tcp-conn-{}", i + 10),
            })
            .sequence(45 + i);

        if i % 3 == 0 {
            builder = builder.warning("Connection timeout detected");
        }

        events.push(builder.build());
    }

    events
}

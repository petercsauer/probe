# prb-core

The foundational crate of the PRB ecosystem, providing the shared data model, error types, and trait definitions that all other crates build upon. Every debug event captured or decoded by PRB flows through the types defined here — from raw packet metadata to fully correlated conversation trees.

## Key Types

| Type | Description |
|------|-------------|
| `DebugEvent` | The central data record: timestamp, source/destination, transport kind, payload, and metadata |
| `DebugEventBuilder` | Ergonomic builder for constructing `DebugEvent` instances |
| `Timestamp` | Nanosecond-precision timestamp used across the pipeline |
| `TransportKind` | Enum of supported transports (gRPC, ZMTP, DDS-RTPS, …) |
| `Payload` | Event payload — raw bytes, decoded text, or structured JSON |
| `NetworkAddr` | IP:port address pair |
| `CorrelationKey` | Key used to group events into conversations |
| `Direction` | Request vs. Response indicator |
| `EventSource` | Origin of the event (pcap file, live capture, fixture, …) |
| `TraceContext` | Extracted OpenTelemetry / B3 / Uber distributed trace context |
| `Conversation` | A correlated group of request/response events |
| `ConversationEngine` | Groups a stream of events into conversations using pluggable strategies |
| `AggregateMetrics` | Computed statistics across a set of events |

## Core Traits

| Trait | Description |
|-------|-------------|
| `CaptureAdapter` | Produces an iterator of `DebugEvent`s from any source (pcap, fixture, live) |
| `ProtocolDecoder` | Decodes raw bytes from a reassembled stream into `DebugEvent`s |
| `CorrelationStrategy` | Groups events into conversations by protocol-specific logic |
| `SchemaResolver` | Resolves protobuf message type names to schema descriptors |
| `EventNormalizer` | Normalizes raw packet data into `DebugEvent`s |
| `Flow` | Represents a bidirectional network flow |

## Usage

```rust
use prb_core::{DebugEventBuilder, TransportKind, Direction, Timestamp, EventSource, Payload};

let event = DebugEventBuilder::new()
    .timestamp(Timestamp::from_nanos(1_700_000_000_000_000_000))
    .transport(TransportKind::Grpc)
    .direction(Direction::Request)
    .source(EventSource {
        adapter: "example".to_string(),
        origin: "10.0.0.1:50051".to_string(),
        network: Some("10.0.0.1:50051".parse().unwrap()),
    })
    .payload(Payload::Raw(vec![].into()))
    .build();
```

## Relationship to Other Crates

`prb-core` has **no dependencies** on other PRB crates — it is the leaf of the dependency tree. Every other crate in the workspace (`prb-pcap`, `prb-grpc`, `prb-storage`, `prb-cli`, etc.) depends on `prb-core` for its type definitions and trait contracts.

See the [PRB documentation](../../docs/) for the full user guide.

<!-- cargo-rdme start -->

### prb-core

The foundational crate for the probe network debugging toolkit.

This crate provides core types and traits used throughout the probe ecosystem:
- [`DebugEvent`]: The universal event type representing protocol messages
- [`ProtocolDecoder`]: Trait for implementing protocol decoders
- [`CaptureAdapter`]: Trait for packet capture sources
- [`ConversationEngine`]: Reconstructs logical conversations from events
- [`TraceContext`]: OpenTelemetry distributed trace context

### Examples

Creating a debug event:

```rust
use prb_core::{DebugEvent, EventSource, TransportKind, Direction, Payload};
use bytes::Bytes;

let event = DebugEvent::builder()
    .source(EventSource {
        adapter: "test".to_string(),
        origin: "example".to_string(),
        network: None,
    })
    .transport(TransportKind::Grpc)
    .direction(Direction::Outbound)
    .payload(Payload::Raw {
        raw: Bytes::from("test data"),
    })
    .build();

assert_eq!(event.transport, TransportKind::Grpc);
assert_eq!(event.warnings.len(), 0);
```

<!-- cargo-rdme end -->

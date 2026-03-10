# prb-zmq

ZMTP (ZeroMQ Message Transport Protocol) decoder for PRB, implementing the `ProtocolDecoder` trait. This crate parses ZMTP 3.0/3.1 wire protocol from reassembled TCP streams, handling the full connection lifecycle: greeting exchange, security mechanism handshake (NULL, PLAIN, CURVE), socket type detection, multipart message reassembly, metadata and identity extraction, topic parsing for PUB/SUB patterns, and graceful degradation for mid-stream captures.

## Key Types

| Type | Description |
|------|-------------|
| `ZmqDecoder` | Implements `ProtocolDecoder` — decodes reassembled TCP bytes into ZMQ `DebugEvent`s |
| `ZmqCorrelationStrategy` | Implements `CorrelationStrategy` — correlates ZMQ messages by socket pair and topic |
| `ZmqError` | Error type for greeting, handshake, and frame parsing failures |

### Internal Modules

| Module | Description |
|--------|-------------|
| `parser` | Low-level ZMTP frame parser: greeting bytes, mechanism negotiation, command/message frames |

## Usage

```rust
use prb_zmq::ZmqDecoder;
use prb_core::ProtocolDecoder;

let mut decoder = ZmqDecoder::new();
let events = decoder.decode_stream(&reassembled_bytes, &flow_context)?;

for event in &events {
    println!("[{}] {} → {}", event.transport, event.source_addr, event.dest_addr);
    if let Some(topic) = event.metadata.get("zmq.topic") {
        println!("  topic: {}", topic);
    }
}
```

## Relationship to Other Crates

`prb-zmq` depends on `prb-core` for the `ProtocolDecoder` and `CorrelationStrategy` traits. It is used by `prb-pcap` as a builtin protocol decoder (behind the `builtin-decoders` feature) and can also be loaded via the plugin system. The `prb-detect` crate identifies ZMTP traffic by its greeting signature and routes streams to this decoder.

See the [PRB documentation](../../docs/) for the full user guide.

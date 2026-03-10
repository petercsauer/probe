# prb-grpc

gRPC and HTTP/2 protocol decoder for PRB, implementing the `ProtocolDecoder` trait. This crate parses reassembled TCP streams to extract HTTP/2 frames, decompress HPACK headers, identify gRPC method paths, extract length-prefixed protobuf payloads (with gzip decompression support), parse trailers and status codes, and gracefully degrade on mid-stream captures where the HTTP/2 connection preface is missing.

## Key Types

| Type | Description |
|------|-------------|
| `GrpcDecoder` | Implements `ProtocolDecoder` — decodes reassembled TCP bytes into gRPC `DebugEvent`s |
| `GrpcCorrelationStrategy` | Implements `CorrelationStrategy` — correlates gRPC requests/responses by HTTP/2 stream ID and method path |
| `GrpcError` | Error type for frame parsing, HPACK, and payload extraction failures |

### Internal Modules

| Module | Description |
|--------|-------------|
| `h2` | HTTP/2 frame parser (DATA, HEADERS, SETTINGS, WINDOW_UPDATE, etc.) and HPACK decoder |
| `lpm` | Length-prefixed message extractor for gRPC's 5-byte framing (compressed flag + 4-byte length) |

## Usage

```rust
use prb_grpc::GrpcDecoder;
use prb_core::ProtocolDecoder;

let mut decoder = GrpcDecoder::new();
let events = decoder.decode_stream(&reassembled_bytes, &flow_context)?;

for event in &events {
    println!("[{}] {} → {}", event.transport, event.source_addr, event.dest_addr);
    if let Some(method) = event.metadata.get("grpc.method") {
        println!("  method: {}", method);
    }
}
```

## Relationship to Other Crates

`prb-grpc` depends on `prb-core` for the `ProtocolDecoder` and `CorrelationStrategy` traits, and on `prb-decode` for protobuf payload decoding. It is used by `prb-pcap` as a builtin protocol decoder (behind the `builtin-decoders` feature) and can also be loaded as a decoder by the plugin system. The `prb-detect` crate identifies gRPC traffic on the wire and routes it to this decoder.

See the [PRB documentation](../../docs/) for the full user guide.

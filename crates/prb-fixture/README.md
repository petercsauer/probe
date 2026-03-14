# prb-fixture

A JSON fixture adapter for PRB that implements the `CaptureAdapter` trait. This crate reads structured JSON files containing arrays of debug events and converts them into the PRB event pipeline. It is primarily used for testing, demos, and offline analysis of hand-crafted or exported event data without requiring a real packet capture.

## Key Types

| Type | Description |
|------|-------------|
| `JsonFixtureAdapter` | Implements `CaptureAdapter` — reads a fixture file and yields `DebugEvent`s |
| `FixtureFile` | Top-level JSON structure containing metadata and an events array |
| `FixtureEvent` | A single event record in the fixture format (timestamps, addresses, payload, metadata) |
| `FixtureSource` | Describes the origin of the fixture (tool version, capture date, description) |
| `FixtureError` | Error type for parsing and validation failures |

## Usage

```rust
use prb_fixture::JsonFixtureAdapter;
use prb_core::CaptureAdapter;

let mut adapter = JsonFixtureAdapter::new("tests/data/grpc_exchange.json".into());
for event in adapter.ingest() {
    let event = event?;
    println!("{:?}", event.transport);
}
```

A fixture file looks like:

```json
{
  "source": { "tool": "prb", "description": "gRPC sample" },
  "events": [
    {
      "timestamp": "2024-01-15T10:30:00Z",
      "transport": "gRPC",
      "direction": "request",
      "src": "10.0.0.1:50051",
      "dst": "10.0.0.2:8080",
      "payload": "base64-encoded-bytes"
    }
  ]
}
```

## Relationship to Other Crates

`prb-fixture` depends on `prb-core` for the `CaptureAdapter` trait, `DebugEvent`, and related types. It is used by `prb-cli` as one of the ingestion backends (alongside `prb-pcap` for packet captures) and is heavily used across the test suites of other crates to provide deterministic event data.

See the [PRB documentation](../../docs/) for the full user guide.

<!-- cargo-rdme start -->

JSON fixture file adapter for PRB.

This crate provides a `CaptureAdapter` implementation that reads debug events
from JSON fixture files for testing and offline analysis.

<!-- cargo-rdme end -->

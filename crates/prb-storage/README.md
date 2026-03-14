# prb-storage

MCAP-based persistent storage layer for PRB debug sessions. This crate serializes and deserializes `DebugEvent` streams into the [MCAP](https://mcap.dev/) container format, enabling session-based capture, replay, and analysis of protocol traffic with embedded schemas and session metadata.

## Key Types

| Type | Description |
|------|-------------|
| `SessionWriter` | Writes `DebugEvent`s to an MCAP file with per-transport channels and embedded schemas |
| `SessionReader` | Reads `DebugEvent`s back from an MCAP file, with channel filtering support |
| `SessionMetadata` | Session-level metadata: capture time, source file, event count, duration |
| `ChannelInfo` | Describes an MCAP channel (topic name, schema, message encoding) |
| `StorageError` | Error type for read/write and format failures |

## Usage

```rust
use prb_storage::{SessionWriter, SessionReader};

// Write events to an MCAP session
let mut writer = SessionWriter::new("session.mcap")?;
for event in events {
    writer.write_event(&event)?;
}
writer.finish()?;

// Read events back
let reader = SessionReader::open("session.mcap")?;
for event in reader.events() {
    println!("{:?}", event?.transport);
}
```

## Relationship to Other Crates

`prb-storage` depends on `prb-core` for event types and `prb-schema` for embedding protobuf descriptors into MCAP files. It is used by `prb-cli` whenever MCAP output is requested (`prb ingest -o session.mcap`) and by the TUI and export commands to read stored sessions. The MCAP format allows other tools in the robotics and observability ecosystem to consume PRB sessions directly.

See the [PRB documentation](../../docs/) for the full user guide.

<!-- cargo-rdme start -->

MCAP-backed storage layer for `DebugEvent` sessions.

This crate provides persistent storage for `DebugEvents` using the MCAP format,
enabling session-based analysis of captured protocol traffic.

<!-- cargo-rdme end -->

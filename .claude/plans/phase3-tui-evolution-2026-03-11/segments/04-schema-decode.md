---
segment: 4
title: "Schema-Aware Decode Pipeline"
depends_on: []
risk: 6
complexity: High
cycle_budget: 10
status: pending
commit_message: "feat(prb-tui): schema-aware protobuf decode — load .proto/.desc, on-demand decode, wire-format fallback"
---

# Segment 4: Schema-Aware Decode Pipeline

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Wire the existing `prb-schema` and `prb-decode` crates into the TUI so that gRPC payloads show decoded field names instead of raw hex bytes. This is the #1 feature gap vs Wireshark.

**Depends on:** None (uses existing prb-schema, prb-decode crates)

## Current State

- `prb-schema` has `SchemaRegistry` that loads `.proto` files (via protox), `.desc` files, and extracts schemas from MCAP
- `prb-decode` has `decode_with_schema(bytes, MessageDescriptor) -> DecodedMessage` and `decode_wire_format(bytes) -> WireMessage`
- The gRPC decoder always emits `Payload::Raw(bytes)`, never `Payload::Decoded`
- The decode tree pane already renders `Payload::Decoded { fields, schema_name }` when present, but no events arrive decoded
- Result: users always see raw hex for gRPC payloads

## Architecture

```
CLI flags: --proto ./protos/ --descriptor-set ./schemas.desc
                          │
                          ▼
               ┌─────────────────────┐
               │   SchemaRegistry    │
               │   (loaded at init)  │
               └─────────┬───────────┘
                          │
    User selects event    │
          │               │
          ▼               ▼
    ┌─────────────────────────────┐
    │  on_event_selected()        │
    │  1. Check Payload::Raw      │
    │  2. Lookup gRPC method →    │
    │     message type            │
    │  3. decode_with_schema()    │
    │  4. Replace with Decoded    │
    │  5. Decode tree re-renders  │
    └─────────────────────────────┘
```

## Scope

- `crates/prb-tui/Cargo.toml` — Add `prb-schema`, `prb-decode` dependencies
- `crates/prb-tui/src/app.rs` — Accept schema registry, on-demand decode on event selection
- `crates/prb-tui/src/lib.rs` — Re-export schema types
- `crates/prb-tui/src/loader.rs` — Schema loading from CLI flags, MCAP auto-extract
- `crates/prb-tui/src/panes/decode_tree.rs` — Enhanced rendering for decoded + wire-format payloads
- `crates/prb-cli/src/commands/tui.rs` — Add `--proto` and `--descriptor-set` CLI flags

## Implementation

### 4.1 Add Dependencies

In `prb-tui/Cargo.toml`:

```toml
prb-schema = { path = "../prb-schema" }
prb-decode = { path = "../prb-decode" }
```

### 4.2 Schema Loading

In `loader.rs`, add a schema loading function:

```rust
use prb_schema::SchemaRegistry;

pub fn load_schemas(
    proto_paths: &[PathBuf],
    descriptor_sets: &[PathBuf],
) -> Result<SchemaRegistry> {
    let mut registry = SchemaRegistry::new();
    for path in descriptor_sets {
        registry.load_descriptor_set_file(path)?;
    }
    if !proto_paths.is_empty() {
        registry.load_proto_files(proto_paths)?;
    }
    Ok(registry)
}
```

For MCAP files, auto-extract schemas during `load_events`:

```rust
if is_mcap {
    let reader = SessionReader::open(path)?;
    if let Ok(schemas) = reader.extract_schemas() {
        // Feed into registry
    }
}
```

### 4.3 Wire into App

Add `Option<SchemaRegistry>` to `AppState`:

```rust
pub struct AppState {
    pub store: EventStore,
    pub schema_registry: Option<SchemaRegistry>,
    // ... existing fields
}
```

In `App::new`, accept optional schema params. In the CLI `tui.rs` command, parse `--proto` and `--descriptor-set` flags and pass through.

### 4.4 On-Demand Decode

When the user selects an event (in `process_action` for `Action::SelectEvent`), attempt schema decode:

```rust
fn try_decode_event(&self, event: &mut DebugEvent) {
    let registry = match &self.state.schema_registry {
        Some(r) => r,
        None => return,
    };

    if let Payload::Raw(ref bytes) = event.payload {
        // Try schema-backed decode first
        if let Some(method) = event.metadata.get("grpc.method") {
            if let Some(msg_desc) = registry.get_message(method) {
                if let Ok(decoded) = prb_decode::decode_with_schema(bytes, &msg_desc) {
                    event.payload = Payload::Decoded {
                        raw: bytes.clone(),
                        fields: decoded.into_fields(),
                        schema_name: Some(method.clone()),
                    };
                    return;
                }
            }
        }

        // Fallback: wire-format decode (field numbers only)
        if let Ok(wire_msg) = prb_decode::decode_wire_format(bytes) {
            // Convert wire fields to a generic decoded representation
            // Show field numbers instead of names
        }
    }
}
```

This is lazy — only decode when the user views an event, not upfront for all events.

### 4.5 Wire-Format Fallback in Decode Tree

When no schema is available but wire-format decode succeeds, render as:

```
Payload (wire-format, no schema)
├─ field 1 (varint): 42
├─ field 2 (length-delimited): "hello"
└─ field 3 (length-delimited): <12 bytes>
```

This requires enhancing `decode_tree.rs` to handle a new intermediate format between Raw and fully Decoded.

### 4.6 Schema Status in Status Bar

Show schema status in the status bar when schemas are loaded:

```
4 events │ gRPC: 2 (decoded) ZMQ: 1 TCP: 1 │ schemas: 3 types │ ...
```

Or when no schemas: `Press P to load .proto files`

### 4.7 CLI Integration

In `crates/prb-cli/src/commands/tui.rs`, add:

```rust
#[arg(long, value_name = "PATH")]
proto: Vec<PathBuf>,

#[arg(long, value_name = "PATH")]
descriptor_set: Vec<PathBuf>,
```

Pass these to the loader and App constructor.

## Key Files and Context

- `crates/prb-schema/src/registry.rs` — `SchemaRegistry::new()`, `load_proto_files()`, `load_descriptor_set_file()`, `get_message()`
- `crates/prb-decode/src/schema_backed.rs` — `decode_with_schema(bytes, MessageDescriptor) -> DecodedMessage`
- `crates/prb-decode/src/wire_format.rs` — `decode_wire_format(bytes) -> WireMessage`
- `crates/prb-core/src/event.rs` — `Payload::Raw(Bytes)`, `Payload::Decoded { raw, fields, schema_name }`
- `crates/prb-tui/src/panes/decode_tree.rs` — Already handles `Payload::Decoded` rendering
- `crates/prb-storage/src/lib.rs` — `SessionReader::extract_schemas()` for MCAP

## Pre-Mortem Risks

- `prb-schema` may have compilation dependencies (protox) that slow builds — use `cargo check -p prb-tui` first
- Method-to-message-type mapping may need the full gRPC service descriptor, not just message name
- Wire-format decode may produce ambiguous results (varint vs zigzag) — present both interpretations

## Build and Test Commands

- Build: `cargo check -p prb-tui`
- Test (targeted): `cargo nextest run -p prb-tui -p prb-schema -p prb-decode`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Dependencies:** `prb-schema` and `prb-decode` are wired into `prb-tui`
2. **CLI flags:** `--proto` and `--descriptor-set` flags accepted by `prb tui` command
3. **Schema loading:** Schemas load from proto files, descriptor sets, and MCAP auto-extraction
4. **On-demand decode:** Selecting a gRPC event with available schema shows decoded field names in decode tree
5. **Wire-format fallback:** When no schema available, wire-format decode shows field numbers instead of pure hex
6. **Status bar:** Shows schema count when loaded
7. **Targeted tests:** New tests for schema loading and decode pipeline pass
8. **Regression tests:** `cargo nextest run --workspace` — no regressions
9. **Full build gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`

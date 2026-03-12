---
segment: 05
title: Schema-Aware Decode Pipeline
depends_on: []
risk: 6
complexity: High
cycle_budget: 10
estimated_lines: 700
---

# Segment 05: Schema-Aware Decode Pipeline

## Context

The TUI currently shows raw hex dumps and basic decode trees. Adding schema-aware decoding (Protobuf, Flatbuffers, etc.) will enable rich, structured views of complex payloads.

## Current State

- Decode tree shows basic field extraction
- No Protobuf schema integration
- No wire-format deserialization with schema

## Goal

Integrate schema-based decoding so users can load .proto/.desc files and see fully decoded messages in decode tree, with field names, types, and nested structures.

## Exit Criteria

1. [ ] `--proto` flag loads .proto files into TUI
2. [ ] `--descriptor-set` flag loads .desc files
3. [ ] Decode tree shows schema-aware fields for Protobuf messages
4. [ ] Field names from schema appear in decode tree
5. [ ] Nested messages expand correctly
6. [ ] Unknown/invalid messages gracefully fall back to raw decode
7. [ ] MCAP files with embedded schemas auto-load
8. [ ] Schema indicator in status bar
9. [ ] Manual test: load gRPC capture with .proto file

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/loader.rs` (~200 lines)
  - Add schema loading from --proto/--descriptor-set
  - Build SchemaRegistry
- `crates/prb-tui/src/panes/decode_tree.rs` (~400 lines)
  - Schema-aware field rendering
  - Protobuf message decoding with schema
  - Nested message expansion
- `crates/prb-tui/src/app.rs` (~100 lines)
  - Pass schema registry to decode tree
  - Status bar schema indicator

### Schema Pipeline

```rust
// Load schemas at startup
let schema_registry = load_schemas(&args.proto, &args.descriptor_set, mcap_path)?;

// Pass to TUI
let mut app = App::new(store, schema_registry);

// In decode tree, use schema for rich decoding
if let Some(schema) = registry.get_message_schema(&message_type) {
    decode_with_schema(payload, schema)
} else {
    decode_raw(payload)
}
```

### Integration Points

- prb-grpc crate already has Protobuf support
- prb-schema crate may need enhancement
- DecodeTree widget needs schema-aware rendering

## Test Plan

1. Prepare test .proto file
2. Launch TUI: `prb tui --proto test.proto grpc.pcap`
3. Verify messages decode with field names
4. Test nested messages
5. Test unknown messages (should fall back gracefully)
6. Test MCAP with embedded schemas
7. Run test suite: `cargo nextest run -p prb-tui -p prb-schema`

## Blocked By

None - foundational feature for Wave 2.

## Blocks

- S08 (Hex & Decode Enhance) - benefits from schema awareness
- S09 (Trace Correlation) - needs schema for structured data
- S17 (Session & TLS) - may use schema for session metadata
- S19 (Plugin System UI) - plugins may provide schemas

## Rollback Plan

If schema integration breaks existing decoding, feature-gate behind `--schema-decode` flag.

## Success Metrics

- Protobuf messages decode with field names
- Clean fallback for unknown messages
- No crashes on malformed schemas
- Good performance (no lag on large schemas)
- Zero regressions in existing tests

## Notes

- Schema loading should be lazy where possible
- Consider caching decoded messages
- May want schema reloading without restart
- Could add schema validation/linting

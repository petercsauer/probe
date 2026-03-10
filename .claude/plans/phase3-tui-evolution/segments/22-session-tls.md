---
segment: 22
title: "Session & TLS Management"
depends_on: [4, 11]
risk: 4
complexity: Medium
cycle_budget: 5
status: pending
commit_message: "feat(prb-tui): MCAP session metadata, auto-schema extract, TLS keylog loading"
---

# Segment 22: Session & TLS Management

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Add session info display for MCAP files, auto-extract embedded schemas, save filtered views, and enable TLS decryption via keylog files.

**Depends on:** S04 (Schema Decode — schema registry to populate), S11 (Live Capture — TLS decryption during ingest)

## Scope

- `crates/prb-tui/src/overlays/session_info.rs` — **New file.** Session metadata overlay
- `crates/prb-tui/src/loader.rs` — MCAP auto-schema extraction, TLS keylog loading
- `crates/prb-cli/src/commands/tui.rs` — `--tls-keylog` flag

## Implementation

### 22.1 Session Info Overlay

Press `i` to show session metadata:

```
Session Info ────────────────────────────
 File: capture.mcap
 Captured: 2026-03-10 14:32:01
 Duration: 5m 23s
 Events: 12,345
 Channels: 3 (gRPC, ZMQ, TCP)
 Schemas: 5 protobuf types embedded
```

Use `SessionReader::metadata()` for MCAP. For other formats, show file size, event count, time range.

### 22.2 Auto-Extract Schemas from MCAP

When loading MCAP, call `SessionReader::extract_schemas()` and feed into `SchemaRegistry`:

```rust
if is_mcap {
    let reader = SessionReader::open(&path)?;
    if let Ok(schemas) = reader.extract_schemas() {
        for schema in schemas {
            registry.load_descriptor_set(&schema)?;
        }
    }
}
```

This means MCAP files with embedded protos auto-decode with zero configuration.

### 22.3 Save Filtered View

Enhance the export dialog (S10) to include MCAP as output format. Use `SessionWriter` to save current filtered events with schema embedding.

### 22.4 TLS Keylog Loading

Add `--tls-keylog` flag:
```bash
prb tui capture.pcap --tls-keylog ./sslkeys.log
```

Load key material via `TlsKeyStore` from `prb-pcap`. Pass to the pcap adapter during loading to enable TLS decryption.

Show TLS status in status bar:
```
12,345 events | TLS: 23/30 streams decrypted | ...
```

## Key Files and Context

- `crates/prb-storage/src/lib.rs` — `SessionReader`, `SessionWriter`, `SessionMetadata`
- `crates/prb-pcap/src/tls.rs` — `TlsKeyStore`, `TlsStreamProcessor`
- `crates/prb-schema/src/registry.rs` — `SchemaRegistry`

## Exit Criteria

1. **Session info:** `i` shows MCAP metadata overlay
2. **Auto-schema:** MCAP with embedded protos auto-decodes
3. **TLS keylog:** `--tls-keylog` enables TLS decryption during load
4. **TLS status:** Status bar shows decryption stats
5. **Tests pass:** `cargo nextest run -p prb-tui`
6. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`

---
segment: 25
title: "Plugin System in TUI"
depends_on: [4]
risk: 5
complexity: Medium
cycle_budget: 7
status: pending
commit_message: "feat(prb-tui): plugin management UI, custom protocol decoders, custom columns"
---

# Segment 25: Plugin System in TUI

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Wire the existing plugin framework (`prb-plugin-api`, `prb-plugin-native`, `prb-plugin-wasm`) into the TUI with a management UI and support for custom protocol decoder columns.

**Depends on:** S04 (Schema Decode — decode pipeline to extend with plugins)

## Current State

- `prb-plugin-api` defines the plugin trait interface
- `prb-plugin-native` loads native shared library plugins
- `prb-plugin-wasm` loads WASM plugins
- No plugin integration in the TUI

## Scope

- `crates/prb-tui/Cargo.toml` — Add plugin dependencies
- `crates/prb-tui/src/overlays/plugin_manager.rs` — **New file.** Plugin management UI
- `crates/prb-tui/src/app.rs` — Wire plugin loading, Ctrl+P shortcut

## Implementation

### 25.1 Plugin Management Overlay

Press `Ctrl+P` to open plugin management:

```
Plugins ────────────────────────────────
 [x] grpc-decode (native, built-in)
 [x] zmq-decode (native, built-in)
 [ ] custom-rpc (wasm, ~/.prb/plugins/)
 ──────────────────────────────────────
 i: info  a: add  r: remove  q: close
```

List loaded plugins with their type (native/wasm) and status. Allow enabling/disabling.

### 25.2 Plugin Loading

On startup, scan `~/.prb/plugins/` for native (.so/.dylib) and WASM (.wasm) plugins. Load enabled plugins into the decode pipeline.

### 25.3 Custom Protocol Columns

Plugins can register custom metadata fields that appear as selectable columns in the event list. Map plugin-provided fields to the configurable column system (from S19).

### 25.4 Plugin Info

Press `i` on a selected plugin to see details: version, author, supported protocols, custom fields.

## Key Files and Context

- `crates/prb-plugin-api/src/lib.rs` — Plugin trait interface
- `crates/prb-plugin-native/src/lib.rs` — Native plugin loader
- `crates/prb-plugin-wasm/src/lib.rs` — WASM plugin loader

## Exit Criteria

1. **Plugin overlay:** Ctrl+P shows plugin management dialog
2. **Plugin listing:** Shows loaded plugins with type and status
3. **Plugin loading:** Scans ~/.prb/plugins/ for plugins on startup
4. **Custom columns:** Plugin-provided fields available as event list columns
5. **Tests pass:** `cargo nextest run -p prb-tui`
6. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`

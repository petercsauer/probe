---
segment: 10
title: "Export & Clipboard"
depends_on: [5]
risk: 3
complexity: Medium
cycle_budget: 5
status: pending
commit_message: "feat(prb-tui): export dialog, copy mode, save filtered view"
---

# Segment 10: Export & Clipboard

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Add export dialog for saving events in multiple formats, copy mode for quick clipboard operations, and save-filtered-view for extracting subsets.

**Depends on:** S05 (Zoom/Mouse — overlay pattern)

## Current State

- `prb-export` supports CSV, HAR, HTML, OTLP, Parquet via `create_exporter(format)`
- `supported_formats()` returns available format names
- No export functionality in the TUI
- No clipboard support

## Scope

- `crates/prb-tui/Cargo.toml` — Add `prb-export` dependency
- `crates/prb-tui/src/overlays/export_dialog.rs` — **New file.** Export format picker
- `crates/prb-tui/src/app.rs` — Wire export dialog, copy mode

## Implementation

### 10.1 Export Dialog

Press `e` to open export dialog:

```
┌─ Export ──────────────────────────┐
│  Format:                          │
│  > JSON  (single event)           │
│    JSON  (all filtered: 42)       │
│    CSV   (all filtered: 42)       │
│    HAR   (gRPC conversations)     │
│    HTML  (report)                 │
│                                   │
│  Output: ./export.json            │
│                                   │
│  Enter: export  Esc: cancel       │
└───────────────────────────────────┘
```

Navigate with j/k, Enter to export, tab to edit output path. Use `prb_export::create_exporter(format)` and call `exporter.export(events, writer)`.

### 10.2 Copy Mode

Press `y` to enter copy mode (keybind submenu):
- `y` again — copy selected event as JSON to clipboard
- `h` — copy hex dump of selected event
- `d` — copy decoded tree as indented text
- `a` — copy source address
- `Esc` — cancel copy mode

Use OSC 52 escape sequence for clipboard:
```rust
fn osc52_copy(text: &str) {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    print!("\x1b]52;c;{}\x07", encoded);
}
```

Add `base64` dependency if not already present.

### 10.3 Save Filtered View

Press `w` to write current filtered events to a file. Show a simple file path input, then write using `serde_json` for JSON or `SessionWriter` for MCAP.

### 10.4 Export Progress

For large exports, show progress bar. Run export in background task if possible.

## Key Files and Context

- `crates/prb-export/src/lib.rs` — `create_exporter()`, `supported_formats()`, `Exporter` trait
- `crates/prb-tui/src/app.rs` — Overlay rendering, InputMode
- `crates/prb-tui/src/event_store.rs` — `events()` to get all events

## Build and Test Commands

- Build: `cargo check -p prb-tui`
- Test (targeted): `cargo nextest run -p prb-tui`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Export dialog:** `e` opens format picker, Enter exports to chosen format
2. **JSON export:** Single event and filtered-set JSON export works
3. **CSV export:** Filtered events export as CSV
4. **Copy mode:** `y` submenu copies event/hex/tree/address to clipboard via OSC 52
5. **Save filtered:** `w` saves current filtered view to file
6. **Tests:** Export dialog navigation and format selection tests pass
7. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`

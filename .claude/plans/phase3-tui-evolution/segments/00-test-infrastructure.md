---
segment: 0
title: "Test Infrastructure Uplift"
depends_on: []
risk: 1
complexity: Low
cycle_budget: 3
status: pending
commit_message: "test(prb-tui): add insta snapshot testing + assert_buffer_lines helpers + upgrade weak assertions"
---

# Segment 0: Test Infrastructure Uplift

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Establish a proper three-tier testing strategy for the TUI before any Phase 3 features land,
so that every subsequent segment can include meaningful visual regression tests in its exit criteria.

**Depends on:** None (pure test infrastructure — no production code changes)

## Current State

- 14 test files exist under `crates/prb-tui/tests/`
- `app.rs` exposes `test_render_to_buffer`, `test_handle_key`, `test_process_action`, `get_input_mode` etc. as `#[doc(hidden)]` test hooks
- Tests render directly to `ratatui::buffer::Buffer` — correct for widget unit tests per ratatui docs
- Assertions are weak: "was there any non-space cell", "was there a digit somewhere", "was there a `[` character"
- No snapshot tests — no baseline for layout regression
- No color/style assertions on individual cells
- `insta` not present as a dev-dependency

## The Three-Tier Strategy

| Tier | Tool | Scope | What it catches |
|------|------|-------|-----------------|
| **1 — Pane unit tests** | `Buffer::assert_eq` / `backend.assert_buffer_lines` | Single pane, small buffer (≤80×10) | Wrong text, wrong position within a pane |
| **2 — Full-app snapshots** | `TestBackend` + `insta::assert_snapshot!` | Whole terminal (120×40) | Any element moving, resizing, or disappearing |
| **3 — Style/color assertions** | `buffer[(x,y)].fg` / `assert_buffer_lines` with styled `Line` | Individual cells | Wrong protocol color, wrong bold/dim attribute |

## Scope

- `crates/prb-tui/Cargo.toml` — Add `insta = "1"` to `[dev-dependencies]`
- `crates/prb-tui/tests/buf_helpers.rs` — **New file.** Shared test utilities (`row_text`, `cell_fg`, `find_text`)
- `crates/prb-tui/tests/tui_snapshots.rs` — **New file.** Full-app insta snapshot tests
- `crates/prb-tui/tests/hex_dump_render_test.rs` — Upgrade weak assertions to `assert_buffer_lines`
- `crates/prb-tui/tests/app_render_detailed_test.rs` — Upgrade status bar / filter bar assertions to exact row checks

## Implementation

### 0.1 Add `insta` dev-dependency

```toml
# crates/prb-tui/Cargo.toml [dev-dependencies]
insta = "1"
```

### 0.2 Shared buffer helpers (`tests/buf_helpers.rs`)

```rust
//! Shared test utilities for TUI buffer inspection.
use ratatui::{buffer::Buffer, style::Color};

/// Extract a full row from the buffer as a plain string.
pub fn row_text(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width)
        .map(|x| buf[(x, y)].symbol().to_string())
        .collect()
}

/// Extract a rectangular region as a vec of row strings.
pub fn region_text(buf: &Buffer, x: u16, y: u16, w: u16, h: u16) -> Vec<String> {
    (y..y + h)
        .map(|row| (x..x + w).map(|col| buf[(col, row)].symbol().to_string()).collect())
        .collect()
}

/// Find the first (x, y) position where `text` appears on a single row.
pub fn find_text(buf: &Buffer, text: &str) -> Option<(u16, u16)> {
    for y in 0..buf.area.height {
        let row = row_text(buf, y);
        if let Some(x) = row.find(text) {
            return Some((x as u16, y));
        }
    }
    None
}

/// Return the foreground color of a specific cell.
pub fn cell_fg(buf: &Buffer, x: u16, y: u16) -> Color {
    buf[(x, y)].fg
}
```

### 0.3 Snapshot tests (`tests/tui_snapshots.rs`)

Use `TestBackend` + `insta::assert_snapshot!` so the full 120×40 grid is captured as a `.snap` file.
Each snapshot covers a distinct UI state.

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prb_tui::{App, event_store::EventStore};
use prb_core::{DebugEvent, TransportKind, /* ... */};
use ratatui::{backend::TestBackend, Terminal};
use insta::assert_snapshot;

fn render_app(app: &mut App, width: u16, height: u16) -> TestBackend {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| app.test_render_to_buffer(f.area(), f.buffer_mut()))
        .unwrap();
    terminal.into_backend() // insta Display impl renders the grid
}

// ── State 1: Empty store ─────────────────────────────────────────────────────

#[test]
fn snapshot_empty_store_80x24() {
    let mut app = App::new(EventStore::new(vec![]), None);
    let backend = render_app(&mut app, 80, 24);
    assert_snapshot!(backend);
}

// ── State 2: Two events, normal view ─────────────────────────────────────────

#[test]
fn snapshot_two_events_120x40() {
    let events = vec![make_grpc_event(1), make_zmq_event(2)];
    let mut app = App::new(EventStore::new(events), None);
    let backend = render_app(&mut app, 120, 40);
    assert_snapshot!(backend);
}

// ── State 3: Active filter with match count ───────────────────────────────────

#[test]
fn snapshot_active_filter_120x40() {
    let events = vec![make_grpc_event(1), make_zmq_event(2), make_grpc_event(3)];
    let mut app = App::new(EventStore::new(events), Some(r#"transport == "gRPC""#.into()));
    let backend = render_app(&mut app, 120, 40);
    assert_snapshot!(backend);
}

// ── State 4: Help overlay ────────────────────────────────────────────────────

#[test]
fn snapshot_help_overlay_120x40() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None);
    app.test_handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    let backend = render_app(&mut app, 120, 40);
    assert_snapshot!(backend);
}

// ── State 5: Filter input mode ───────────────────────────────────────────────

#[test]
fn snapshot_filter_input_mode_120x40() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None);
    app.test_handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    let backend = render_app(&mut app, 120, 40);
    assert_snapshot!(backend);
}

// ── State 6: Each pane focused (4 snapshots) ─────────────────────────────────

#[test]
fn snapshot_decode_tree_focused_120x40() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None);
    // Tab once → DecodeTree
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let backend = render_app(&mut app, 120, 40);
    assert_snapshot!(backend);
}

// ... similar for HexDump, Timeline
```

On first run, `cargo nextest run -p prb-tui -- snapshot` generates `.snap` files under
`crates/prb-tui/tests/snapshots/`. Run `cargo insta review` to accept them.

### 0.4 Upgrade existing assertions

**`hex_dump_render_test.rs`** — replace "was there a '0' character somewhere" with `assert_buffer_lines`
for the offset column region. Example for the scroll test:

```rust
// Before (weak):
assert!(found_five_in_offset, "Should show offset containing '5'");

// After (precise) — first content row when scrolled to line 5 starts at offset 0x50:
let first_content_row = row_text(&buffer, 1); // row 1 = first data row (row 0 = border)
assert!(first_content_row.starts_with("00000050"), "offset should be 0x50 at scroll=5");
```

**`app_render_detailed_test.rs`** — replace the bracket-scan loop with:

```rust
// Before: scan whole buffer for any '[' character
// After: check status bar row contains exact content
let status = row_text(&buffer, area.height - 1);
assert!(status.contains("2 events"), "status bar event count");
```

## Key Files and Context

- `crates/prb-tui/Cargo.toml` — add `insta` dev-dep
- `crates/prb-tui/tests/` — new files and upgrades
- ratatui 0.30 docs: `TestBackend::assert_buffer_lines` accepts plain strings or styled `Line<'_>` — use styled lines for color assertions
- ratatui docs explicitly recommend: widget unit tests → `Buffer` directly; full TUI integration tests → `TestBackend`
- `cargo insta review` — interactive snapshot review/accept workflow
- `INSTA_UPDATE=new cargo nextest run -p prb-tui` — accept all new snapshots non-interactively

## Build and Test Commands

```bash
# Install cargo-insta (one-time)
cargo install cargo-insta

# Run snapshot tests only
cargo nextest run -p prb-tui -- snapshot

# Accept new/changed snapshots
cargo insta review

# Accept all new snapshots non-interactively (useful in CI for first run)
INSTA_UPDATE=new cargo nextest run -p prb-tui

# Full gate
cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings
```

## Exit Criteria

1. **`insta` in dev-deps:** `crates/prb-tui/Cargo.toml` has `insta = "1"` under `[dev-dependencies]`
2. **`buf_helpers.rs` exists:** `row_text`, `region_text`, `find_text`, `cell_fg` compile and are usable from other test files
3. **6+ snapshot tests:** covering empty store, two-event normal view, active filter, help overlay, filter input mode, and at least one non-default pane focus; `.snap` files committed
4. **Upgraded assertions:** `hex_dump_render_test.rs` scroll test uses exact offset string check; `app_render_detailed_test.rs` status bar test uses `row_text` + string contains
5. **No regressions:** `cargo nextest run -p prb-tui` — all existing tests still pass
6. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`

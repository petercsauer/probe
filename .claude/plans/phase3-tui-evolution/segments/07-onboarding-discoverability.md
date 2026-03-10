---
segment: 7
title: "Onboarding & Discoverability"
depends_on: [1]
risk: 3
complexity: Medium
cycle_budget: 7
status: pending
commit_message: "feat(prb-tui): welcome screen, demo mode, which-key popup, command palette"
---

# Segment 7: Onboarding & Discoverability

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Make the TUI welcoming to first-time users and powerful for experts with progressive disclosure of features.

**Depends on:** S01 (Visual Polish — theme)

## Current State

- TUI launched with no file shows empty panes with no guidance
- Help overlay (`?`) shows flat keybinding list, not scrollable
- No built-in sample data for exploration

## Scope

- `crates/prb-tui/src/overlays/` — **New directory.** welcome.rs, which_key.rs, command_palette.rs
- `crates/prb-tui/src/app.rs` — Wire overlays, demo mode, empty-state rendering
- `crates/prb-tui/src/demo.rs` — **New file.** Built-in sample dataset
- `crates/prb-cli/src/commands/tui.rs` — `--demo` flag

## Implementation

### 7.1 Welcome Screen

When launched with no file, show a centered welcome overlay with usage instructions. Render as a styled `Paragraph` in a centered `Rect`. Dismiss on any key.

### 7.2 Demo Mode

Create `demo.rs` generating ~50 synthetic events via `DebugEventBuilder`:
- 3 gRPC req/resp pairs (one with error status)
- 2 ZMQ pub/sub flows
- 1 DDS-RTPS exchange
- 2 raw TCP connections
- Realistic addresses, timestamps over 5 minutes, some with warnings

Add `--demo` flag to CLI.

### 7.3 Contextual Hints on Empty Panes

When a pane has nothing to display, show helpful hint text (dimmed, centered):
- Decode tree: "Select an event above to see decoded layers"
- Hex dump: "Select an event to view raw bytes"
- Timeline: "Load a capture to see time distribution"

### 7.4 Which-Key Popup

After pressing a prefix key (`f` for quick-filter), show a floating popup listing continuations. Auto-dismiss on selection or Esc. Add `InputMode::WhichKey { prefix, options }`.

### 7.5 Command Palette

Press `:` to open fuzzy-searchable command list. Each command maps to an Action or key sequence. Substring match for filtering. Enter executes, Esc dismisses.

### 7.6 Improved Help Overlay

Make the existing help overlay scrollable (j/k to scroll) and organized by category with visual separators.

## Key Files and Context

- `crates/prb-tui/src/app.rs` — Event loop, overlay rendering, InputMode
- `crates/prb-core/src/event.rs` — `DebugEventBuilder` for demo events
- `crates/prb-cli/src/commands/tui.rs` — CLI argument parsing

## Build and Test Commands

- Build: `cargo check -p prb-tui`
- Test (targeted): `cargo nextest run -p prb-tui`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Welcome screen:** Shown when TUI launched with no file, dismissible on keypress
2. **Demo mode:** `--demo` loads ~50 synthetic events covering all transports
3. **Empty hints:** Each pane shows contextual help text when no data available
4. **Which-key:** Pressing `f` shows popup with quick-filter options
5. **Command palette:** `:` opens searchable command list
6. **Help overlay:** Scrollable, organized by category
7. **Tests:** Demo generation and command palette mapping tests pass
8. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`

---
segment: 8
title: "TUI Snapshot Expansion"
depends_on: []
risk: 4/10
complexity: Low
cycle_budget: 12
status: merged
commit_message: "test(tui): Expand snapshot coverage to 30+ UI states and overlays"
---

# Segment 8: TUI Snapshot Expansion

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Expand TUI snapshot tests from 8 to 30+ covering all critical UI states and input modes.

**Depends on:** None (independent)

## Context: Issues Addressed

**Core Problem:** TUI has comprehensive test coverage (600+ tests) but visual regression coverage is limited. Only 8 snapshot tests exist covering basic states. Missing coverage includes 8 of 11 input modes, 9 of 10+ overlays, and only 3 of 8 panes tested. No error state snapshots, no edge case snapshots (very long lists, wide payloads, small terminals).

**Proposed Fix:** Add 22+ new snapshot tests organized by category (input modes, overlays, panes at multiple terminal sizes, error states, edge cases). Use insta crate for snapshot management and organize snapshots in subdirectories.

**Pre-Mortem Risks:**
- Snapshot churn on minor UI tweaks - acceptable tradeoff, use `cargo insta review` for efficient workflow
- Large number of snapshots adds review burden - mitigate by organizing in subdirectories with clear naming
- Unicode rendering might differ across platforms - document expected platform (macOS/Linux) in snapshot metadata

## Scope

- `crates/prb-tui/tests/tui_snapshots.rs` - Expand from 8 to 30+ tests
- `crates/prb-tui/tests/snapshots/` - Organized snapshot files

## Key Files and Context

**`crates/prb-tui/tests/tui_snapshots.rs`**:
- Currently 8 snapshot tests
- Pattern: `insta::assert_snapshot!(render_app(&mut app, width, height))`

**`crates/prb-tui/tests/buf_helpers.rs`**:
- Test utilities (row_text, find_text, cell_fg)

**Current snapshots cover:**
- Empty state, two events, filtered view, help overlay
- Filter input mode, panes focused (decode tree, hex dump, timeline)

**Missing coverage:**
- 11 input modes (only 3 covered): GoToEvent, Welcome, WhichKey, CommandPalette, PluginManager, ExportDialog, CaptureConfig, ThemeEditor
- 10+ overlays (only 1 covered): Metrics, FollowStream, DiffView, SessionInfo, TlsKeylogPicker, AIFilter, Plugin manager, Export dialog, Capture config, Theme editor
- 8 panes (only 3 covered): Waterfall, AI Panel, TraceCorrelation, ConversationList, plus 120x40 variants
- Error states: Empty store, no filter matches, failed AI, parse error, loading
- Edge cases: Very long list, very wide payload, Unicode, small terminal

## Implementation Approach

1. **Add input mode snapshots** in `tui_snapshots.rs` (11 modes, need 8 more):
   - Normal ✓ (exists)
   - Filter ✓ (exists)
   - Help ✓ (exists)
   - GoToEvent (new) - showing "Go to event:" prompt
   - Welcome (new) - showing welcome screen
   - WhichKey (new) - showing key binding hints
   - CommandPalette (new) - showing command palette
   - PluginManager (new) - showing plugin manager UI
   - ExportDialog (new) - showing export dialog
   - CaptureConfig (new) - showing capture configuration
   - ThemeEditor (new) - showing theme editor

2. **Add overlay snapshots** (10+ overlays, need 9 more):
   - Metrics overlay - showing stats/metrics
   - FollowStream overlay - following TCP/UDP stream
   - DiffView overlay - comparing two events
   - SessionInfo overlay - showing session information
   - TlsKeylogPicker overlay - selecting keylog file
   - AIFilter overlay - AI-based filtering (in progress)
   - Plugin manager overlay
   - Export dialog overlay
   - Capture config overlay
   - Theme editor overlay

3. **Add pane focus snapshots** at 2 terminal sizes (8 panes × 2 sizes = 16, need 13 more):
   - EventList ✓ at 80x24 (exists)
   - DecodeTree ✓ (exists)
   - HexDump ✓ (exists)
   - Timeline ✓ (exists)
   - Waterfall at 80x24 and 120x40 (new)
   - AI Panel at 80x24 and 120x40 (new)
   - TraceCorrelation at 80x24 and 120x40 (new)
   - ConversationList at 80x24 and 120x40 (new)
   - Also add 120x40 variants for EventList, DecodeTree, HexDump, Timeline (new)

4. **Add error state snapshots** (5 new):
   - Empty store with "No events loaded" message
   - Filter with no matches showing "No events match filter"
   - Failed AI explanation with error message
   - Parse error in filter input
   - Loading state (spinner/progress indicator)

5. **Add edge case snapshots** (4 new):
   - Very long event list (1000+ events, test scrolling UI)
   - Very wide payload (hex dump horizontal scroll indicators)
   - Unicode in event data (verify rendering)
   - Extremely small terminal (40x10, verify graceful degradation)

6. **Organize snapshots** by category using insta settings:
   ```rust
   let mut settings = insta::Settings::clone_current();
   settings.set_snapshot_path("../snapshots/input_modes");
   settings.bind(|| {
       insta::assert_snapshot!("normal_mode", render_app(&mut app, 80, 24));
   });
   ```

## Alternatives Ruled Out

- **Snapshot testing with actual terminal output:** Rejected - environment-dependent (terminal emulator, fonts), Buffer snapshots more portable
- **Testing only happy paths:** Rejected - error states are critical UX, users need to see helpful error messages

## Pre-Mortem Risks

- Snapshot churn on minor UI tweaks: Acceptable tradeoff - use `cargo insta review` for efficient review workflow
- Large number of snapshots (30+) adds review burden: Mitigate by organizing in subdirectories and clear naming
- Unicode rendering might differ across platforms: Document expected platform (macOS/Linux) in snapshot metadata

## Build and Test Commands

- Build: `cargo build -p prb-tui`
- Test (targeted): `cargo test -p prb-tui tui_snapshots`
- Test (regression): `cargo test -p prb-tui`
- Test (full gate): `cargo nextest run -p prb-tui`
- Review snapshots: `cargo insta review` (after snapshots update)

## Exit Criteria

1. **Targeted tests:**
   - `tui_snapshots` - 30+ tests pass (8 existing + 22+ new)
   - All 11 input modes covered with snapshots
   - All 10+ overlays covered with snapshots
   - All 8 panes covered at 80x24 and 120x40 terminal sizes
   - 5 error states covered
   - 4 edge cases covered

2. **Regression tests:** All existing TUI tests pass (600+ tests)

3. **Full build gate:** `cargo build -p prb-tui` succeeds with zero warnings

4. **Full test gate:** `cargo nextest run -p prb-tui` passes

5. **Self-review gate:**
   - Snapshots organized by category in subdirectories
   - No duplicate coverage
   - Clear naming convention (e.g., "input_mode_goto_event", "overlay_metrics", "pane_waterfall_80x24")

6. **Scope verification gate:** Only modified:
   - `tui_snapshots.rs` - added 22+ new snapshot tests
   - Snapshot files in `tests/snapshots/` directory
   - No src/ changes

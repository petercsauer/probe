---
segment: 13
title: Live Capture Config UI
depends_on: [09]
risk: 4
complexity: Medium
cycle_budget: 5
estimated_lines: 350
---

# Segment 13: Live Capture Config UI

## Context

Live capture works but requires CLI args. Need UI for interface selection, BPF filter editing, and capture control.

## Goal

Add overlay for configuring live capture: select interface, edit BPF filter, start/stop/pause.

## Exit Criteria

1. [ ] Keybinding `L` opens live capture config overlay
2. [ ] Interface picker with list of available interfaces
3. [ ] BPF filter editor with syntax highlighting
4. [ ] Validation feedback for invalid BPF
5. [ ] Start capture button (or Enter)
6. [ ] Stop/Pause controls
7. [ ] Packet count and stats display
8. [ ] Save/load capture profiles
9. [ ] Manual test: configure and start capture from UI

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/overlays/live_capture_config.rs` (~250 lines NEW)
  - Interface picker
  - BPF filter editor
  - Validation
- `crates/prb-tui/src/app.rs` (~100 lines)
  - Wire overlay
  - Handle capture start/stop

### Interface Picker

```rust
struct LiveCaptureConfig {
    interfaces: Vec<Interface>,
    selected_interface: usize,
    bpf_filter: String,
}

fn list_interfaces() -> Vec<Interface> {
    InterfaceEnumerator::list().unwrap_or_default()
}
```

### BPF Validation

Use libpcap to validate BPF syntax before starting capture.

## Test Plan

1. Press `L` to open overlay
2. Select interface
3. Enter BPF filter
4. Start capture
5. Verify packets flow
6. Stop capture
7. Run test suite

## Blocked By

- S09 (Trace Correlation) - benefits from better capture control

## Blocks

None - config UI is additive.

## Rollback Plan

Remove overlay, use CLI args only.

## Success Metrics

- Interface picker works
- BPF validation accurate
- Capture starts correctly
- Good UX
- Zero regressions

---
segment: 17
title: Session & TLS Management
depends_on: [05]
risk: 4
complexity: Medium
cycle_budget: 5
estimated_lines: 400
---

# Segment 17: Session & TLS Management

## Context

TUI doesn't persist sessions or manage TLS keylog files. Need session save/restore and TLS keylog management.

## Goal

Add session persistence, TLS keylog management, and MCAP session metadata.

## Exit Criteria

1. [ ] Save session with `:save-session <file>`
2. [ ] Restore session with `--session <file>`
3. [ ] Session includes: filter, scroll position, pane focus
4. [ ] TLS keylog picker UI
5. [ ] Decrypt TLS traffic with keylog
6. [ ] Show decryption status in status bar
7. [ ] MCAP metadata support
8. [ ] Manual test: save/restore session, decrypt TLS

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/session.rs` (~200 lines NEW)
  - Session serialization
  - Save/restore logic
- `crates/prb-tui/src/overlays/tls_keylog_picker.rs` (~100 lines NEW)
  - Keylog file picker
- `crates/prb-tui/src/app.rs` (~100 lines)
  - Wire session commands
  - TLS status display

### Session Format

```json
{
  "version": "1.0",
  "input_file": "capture.pcap",
  "filter": "grpc.status != 0",
  "scroll_offset": 42,
  "selected_event": 123,
  "pane_focus": "EventList",
  "tls_keylog": "/path/to/keylog.txt"
}
```

### TLS Keylog

Load keylog and pass to decoder:
```rust
if let Some(keylog) = &self.tls_keylog {
    processor.load_keylog(keylog)?;
}
```

## Test Plan

1. Load capture
2. Apply filter, scroll, navigate
3. Save session
4. Close TUI
5. Restore session
6. Verify state restored
7. Test TLS decryption with keylog
8. Run test suite

## Blocked By

- S05 (Schema Decode) - session may include schema info

## Blocks

None - session management is additive.

## Rollback Plan

Remove session commands, disable TLS picker.

## Success Metrics

- Session save/restore works
- TLS decryption functional
- State fully preserved
- Zero regressions

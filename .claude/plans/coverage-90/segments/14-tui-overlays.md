---
segment: 14
title: TUI overlays testable to 50%
depends_on: [7, 10]
risk: 3
complexity: Medium
cycle_budget: 12
estimated_lines: ~320 test lines
---

# Segment 14: TUI Overlays Testable Logic to 50%

## Context

**Target overlays:**
- `export_dialog.rs` - 31.19% → 60% (71 lines uncovered)
- `command_palette.rs` - 23.36% → 50% (82 lines uncovered)
- `metrics.rs` - 90.11% ✅ (maintain)
- `welcome.rs` - 100% ✅ (maintain)

**Accept <10% for pure UI:**
- `capture_config.rs`, `diff_view.rs`, `follow_stream.rs`, `theme_editor.rs`, `tls_keylog_picker.rs`, `session_info.rs`, `plugin_manager.rs`, `which_key.rs`

## Goal

Test input validation, state machines in dialogs - NOT layout/rendering.

## Implementation Plan

### Priority 1: Export Dialog Logic (~150 lines)

```rust
#[test]
fn test_export_dialog_format_selection() {
    let mut dialog = ExportDialog::new();
    dialog.select_format(ExportFormat::Json);
    assert_eq!(dialog.selected_format(), ExportFormat::Json);
}

#[test]
fn test_export_dialog_path_validation() {
    let mut dialog = ExportDialog::new();
    let result = dialog.validate_path("/invalid\x00path");
    assert!(result.is_err());
}
```

### Priority 2: Command Palette Filtering (~100 lines)

Test command search, fuzzy matching, recents.

### Priority 3: Plugin Manager State (~70 lines)

Test plugin list management, enable/disable logic.

## Success Metrics

- export_dialog: 31.19% → 60%+
- command_palette: 23.36% → 50%+
- ~45 new tests

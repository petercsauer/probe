---
segment: 02
title: Complete Export Dialog
depends_on: []
risk: 2
complexity: Low
cycle_budget: 3
estimated_lines: 200
---

# Segment 02: Complete Export Dialog

## Context

The export dialog overlay exists and is accessible via `e` key. The `ExportDialogOverlay` struct is present, but may be incomplete or untested. Need to verify all export formats work and polish the UX.

## Current State

```rust
// In app.rs:
export_dialog: Option<ExportDialogOverlay>,

KeyCode::Char('e') => {
    // Open export dialog
    self.export_dialog = Some(ExportDialogOverlay::new(...));
}
```

Export formats available from CLI:
- CSV
- HAR (HTTP Archive)
- OTLP (OpenTelemetry)
- Parquet
- HTML

## Goal

Ensure export dialog is complete, functional, and provides good UX for exporting filtered events to all supported formats.

## Exit Criteria

1. [ ] Export dialog renders correctly with all format options
2. [ ] Can select export format via keyboard (arrow keys, Enter)
3. [ ] Can specify output filename
4. [ ] Export executes and writes file with progress indicator
5. [ ] Success/error message displayed
6. [ ] Dialog can be cancelled with Esc
7. [ ] All export formats tested (CSV, HAR, OTLP, Parquet, HTML)
8. [ ] Status message confirms successful export with file path

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/overlays/export_dialog.rs` (~150 lines)
  - Polish dialog rendering
  - Add progress indicator
  - Add success/error handling
- `crates/prb-tui/src/app.rs` (~50 lines)
  - Handle export execution
  - Display status messages

### Export Flow

1. User presses `e`
2. Dialog shows format selection
3. User selects format, enters filename
4. Export executes (may take time for large files)
5. Progress indicator shows status
6. Success/error message displayed
7. Dialog closes

### Format Suggestions

```rust
enum ExportFormat {
    CSV,
    HAR,
    OTLP,
    Parquet,
    HTML,
}

impl ExportFormat {
    fn default_extension(&self) -> &str {
        match self {
            ExportFormat::CSV => "csv",
            ExportFormat::HAR => "har",
            ExportFormat::OTLP => "json",
            ExportFormat::Parquet => "parquet",
            ExportFormat::HTML => "html",
        }
    }
}
```

## Test Plan

1. Open TUI with test pcap
2. Press `e` to open export dialog
3. Select each format and export
4. Verify files are created correctly
5. Test cancellation with Esc
6. Test with filtered events
7. Test with large file (show progress indicator)
8. Run test suite: `cargo nextest run -p prb-tui`

## Blocked By

None - this is Wave 1, independent work.

## Blocks

None - export is standalone feature.

## Rollback Plan

If export fails, dialog can be disabled by removing keybinding or feature-gating.

## Success Metrics

- All export formats work correctly
- Good UX with clear feedback
- No crashes or hangs during export
- Progress indicator for large files
- Zero regressions in existing tests

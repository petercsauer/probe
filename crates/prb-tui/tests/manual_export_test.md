# Manual Export Dialog Test Plan

## Test Procedure

This document outlines the manual testing steps for the export dialog functionality.

### Prerequisites

1. Build the TUI: `cargo build -p prb-tui`
2. Have a test pcap file available (e.g., `tests/fixtures/captures/http/http-chunked-gzip.pcap`)

### Test Cases

#### 1. Export Dialog Rendering
- [ ] Open TUI with test pcap: `cargo run -p prb-tui -- tests/fixtures/captures/http/http-chunked-gzip.pcap`
- [ ] Press `e` to open export dialog
- [ ] Verify dialog displays with title "Export"
- [ ] Verify all format options are visible:
  - JSON (single event)
  - JSON (all filtered: N)
  - CSV (all filtered: N)
  - HAR (gRPC conversations)
  - OTLP (OpenTelemetry)
  - HTML (report)
  - Parquet (columnar format) - if feature enabled

#### 2. Keyboard Navigation
- [ ] Press `j` or Down arrow to move selection down
- [ ] Verify selection highlight moves
- [ ] Press `k` or Up arrow to move selection up
- [ ] Verify selection highlight moves
- [ ] Navigate to end and press Down to verify wrap-around to start
- [ ] Navigate to start and press Up to verify wrap-around to end

#### 3. Output Path Editing
- [ ] In export dialog, verify default path is shown (e.g., "./export.json")
- [ ] Press Tab to enter path editing mode
- [ ] Verify cursor appears in path field
- [ ] Type a new path (e.g., "./test_export.json")
- [ ] Press Enter to confirm
- [ ] Verify path editing mode exits

#### 4. Format Selection Updates Path
- [ ] Navigate between different formats using arrow keys
- [ ] Verify output path extension changes automatically:
  - JSON → ./export.json
  - CSV → ./export.csv
  - HAR → ./export.har
  - OTLP → ./export.json
  - HTML → ./export.html

#### 5. Export Execution - JSON
- [ ] Select "JSON (single event)"
- [ ] Press Enter to export
- [ ] Verify status message shows "Exported 1 events to ./export.json"
- [ ] Verify file was created: `ls -la ./export.json`
- [ ] Verify file contains valid JSON: `cat ./export.json`

#### 6. Export Execution - CSV
- [ ] Open export dialog again (press `e`)
- [ ] Select "CSV (all filtered: N)"
- [ ] Press Enter to export
- [ ] Verify status message shows "Exported N events to ./export.csv"
- [ ] Verify file was created: `ls -la ./export.csv`
- [ ] Verify file contains CSV data: `head ./export.csv`

#### 7. Export Execution - HAR
- [ ] Open export dialog (press `e`)
- [ ] Select "HAR (gRPC conversations)"
- [ ] Press Enter to export
- [ ] Verify status message shows success
- [ ] Verify file was created: `ls -la ./export.har`
- [ ] Verify file contains valid HAR JSON: `cat ./export.har | jq .log`

#### 8. Export Execution - OTLP
- [ ] Open export dialog (press `e`)
- [ ] Select "OTLP (OpenTelemetry)"
- [ ] Press Enter to export
- [ ] Verify status message shows success
- [ ] Verify file was created: `ls -la ./export.json`
- [ ] Verify file contains OTLP structure: `cat ./export.json | jq .resourceSpans`

#### 9. Export Execution - HTML
- [ ] Open export dialog (press `e`)
- [ ] Select "HTML (report)"
- [ ] Press Enter to export
- [ ] Verify status message shows success
- [ ] Verify file was created: `ls -la ./export.html`
- [ ] Open file in browser to verify it renders correctly

#### 10. Dialog Cancellation
- [ ] Open export dialog (press `e`)
- [ ] Press Esc to cancel
- [ ] Verify dialog closes without exporting
- [ ] Verify no new files were created

#### 11. Path Editing Cancellation
- [ ] Open export dialog (press `e`)
- [ ] Press Tab to enter path editing
- [ ] Type some text
- [ ] Press Esc to cancel editing
- [ ] Verify path editing mode exits
- [ ] Verify changes are discarded

#### 12. Error Handling
- [ ] Open export dialog (press `e`)
- [ ] Press Tab and enter an invalid path (e.g., "/root/noaccess/export.csv")
- [ ] Press Enter to attempt export
- [ ] Verify error message is displayed

## Automated Test Coverage

The following aspects are covered by automated tests:

- Export dialog creation and initialization
- Format selection and navigation
- Path editing toggle
- Automatic path extension updates
- All export format execution (JSON, CSV, HAR, OTLP, HTML)
- Empty event export handling
- Unsupported format error handling

See `tests/export_dialog_test.rs` and `tests/export_integration_test.rs` for details.

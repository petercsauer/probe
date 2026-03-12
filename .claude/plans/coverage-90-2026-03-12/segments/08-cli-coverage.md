---
segment: 08
title: prb-cli to 85%
depends_on: [1]
risk: 3
complexity: Medium
cycle_budget: 12
estimated_lines: ~300 test lines
---

# Segment 08: prb-cli Coverage to 85%

## Context

**Current:** 55.06%
**Target:** 85%
**Gap:** +29.94 percentage points

**CRITICAL GAPS:**
- `src/commands/tui.rs` - **0% (185 lines uncovered)**
- `src/commands/capture.rs` - 18.04% (110 lines uncovered)
- `src/main.rs` - 61.97% (21 lines uncovered)

## Goal

Integration tests for all CLI commands: capture, export, ingest, inspect, merge, plugins, schemas, tui.

## Implementation Plan

### Priority 1: Command Handler Tests (~200 lines)

```rust
// crates/prb-cli/tests/command_tests.rs

#[test]
fn test_export_command_with_valid_input() {
    let args = ExportArgs {
        input: "test.mcap",
        output: "test.json",
        format: Format::Json,
    };
    let result = execute_export(&args);
    assert!(result.is_ok());
}

#[test]
fn test_capture_command_validation() {
    let args = CaptureArgs {
        interface: "nonexistent",
        ..Default::default()
    };
    let result = validate_capture_args(&args);
    assert!(result.is_err());
}
```

### Priority 2: Argument Validation (~100 lines)

Test clap argument parsing, validation logic, help text generation.

## Success Metrics

- prb-cli: 55.06% → 85%+
- ~50 new test functions

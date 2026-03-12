# Phase 3 TUI v2: Remaining Work

## Overview

This is a revised Phase 3 plan that focuses on **incomplete TUI features only**. The original Phase 3 plan had 25 segments, but several have already been implemented:

### ✅ Already Complete (Skipped in v2)
- Visual Polish (themes, status bar, focus indicators)
- Zoom & Mouse (pane zoom, mouse selection, resize)
- Filter & Search (filter bar, history, quick filters)
- Live Capture (fully functional)
- Session Comparison (diff mode working)

### 🎯 This Plan: 19 Remaining Segments

This plan completes the TUI evolution with 19 segments across 5 waves:

**Wave 1 - Enable Existing (Quick Wins)**
- S01: Enable Conversation View
- S02: Complete Export Dialog
- S03: Wire AI Panel
- S04: Enable Metrics Overlay

**Wave 2 - Missing Core Features**
- S05: Schema-Aware Decode Pipeline
- S06: Error Intelligence
- S07: Column Layout Improvements
- S08: Hex Dump & Decode Enhancements

**Wave 3 - Advanced Analytics**
- S09: Trace Correlation View
- S10: AI Smart Features
- S11: Timeline Enhancements
- S12: Complete Request Waterfall

**Wave 4 - Config & Performance**
- S13: Live Capture Config UI
- S14: Theme System & Configuration
- S15: Large File Performance

**Wave 5 - Final Polish**
- S16: Accessibility
- S17: Session & TLS Management
- S18: Multi-Tab Support
- S19: Plugin System UI

## Quick Start

### Prerequisites

```bash
# Install orchestrate v2 script
# (assuming it's in your PATH or .claude/scripts/)

# Verify prb builds
cd ~/probe
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

### Run Full Plan

```bash
cd ~/probe/.claude/plans/phase3-tui-v2

# Dry run to see execution order
orchestrate-v2 --dry-run

# Execute all waves in sequence
orchestrate-v2

# Execute specific wave only
orchestrate-v2 --wave 1

# Execute specific segment only
orchestrate-v2 --segment 05
```

### Configuration

Edit `orchestrate.toml` to adjust:
- `max_concurrent` - number of parallel segments (default: 4)
- `cycle_timeout_minutes` - timeout per cycle (default: 30)
- `auto_merge` - auto-merge to main (default: false, requires manual review)

## Execution Flow

The orchestrator will:

1. **Discover segments** from `segments/*.md` files
2. **Group by waves** based on dependencies
3. **Execute in parallel** within each wave (max 4 concurrent)
4. **Create git worktrees** for isolation
5. **Run build/test/clippy** gates for each segment
6. **Generate reports** in `segments/*.report.md`
7. **Merge to main** after wave completion (if auto_merge enabled)
8. **Persist state** in `state.db` for resumability

## Current Status

```bash
# Check overall progress
orchestrate-v2 status

# View specific segment status
orchestrate-v2 status --segment 05

# Resume after interruption
orchestrate-v2 resume
```

## Segment Structure

Each segment file (`segments/NN-*.md`) contains:

```yaml
---
segment: NN
title: Segment Title
depends: [list of segment numbers]
risk: 1-10
complexity: Low|Medium|High
cycle_budget: N
estimated_lines: ~N
---
```

Followed by:
- Context (current state)
- Goal (what to achieve)
- Exit Criteria (checklist)
- Implementation Notes
- Test Plan
- Blocks/Blocked By
- Rollback Plan

## Manual Segment Execution

To work on a segment manually without orchestrator:

```bash
# Create worktree
git worktree add /tmp/phase3-v2-S05 main

# Work in worktree
cd /tmp/phase3-v2-S05

# Make changes, test, commit
cargo build -p prb-tui
cargo nextest run -p prb-tui
cargo clippy -p prb-tui -- -D warnings

# When complete, merge back
cd ~/probe
git merge /tmp/phase3-v2-S05

# Cleanup worktree
git worktree remove /tmp/phase3-v2-S05
```

## All Segments Created ✅

All **19 segments** are complete and ready for execution:

**Wave 1 - Enable Existing (Quick Wins)**
- ✅ `01-enable-conversation.md` - Uncomment conversation view
- ✅ `02-complete-export.md` - Complete export dialog
- ✅ `03-wire-ai-panel.md` - Wire AI panel streaming
- ✅ `04-enable-metrics.md` - Enable metrics overlay

**Wave 2 - Missing Core Features**
- ✅ `05-schema-decode.md` - Protobuf schema integration
- ✅ `06-error-intelligence.md` - Error code explanations
- ✅ `07-column-layout.md` - Adaptive column sizing
- ✅ `08-hex-decode-enhance.md` - Hex search, expand-all

**Wave 3 - Advanced Analytics**
- ✅ `09-trace-correlation.md` - OTel trace correlation
- ✅ `10-ai-smart.md` - AI smart features (NL filters)
- ✅ `11-timeline-enhance.md` - Interactive timeline
- ✅ `12-complete-waterfall.md` - Request waterfall view

**Wave 4 - Config & Performance**
- ✅ `13-live-config-ui.md` - Live capture config UI
- ✅ `14-theme-config.md` - Runtime theme switching
- ✅ `15-large-file-perf.md` - Streaming & virtual scroll

**Wave 5 - Final Polish**
- ✅ `16-accessibility.md` - Colorblind & high contrast
- ✅ `17-session-tls.md` - Session save/restore, TLS
- ✅ `18-multi-tab.md` - Multi-tab support
- ✅ `19-plugin-system-ui.md` - Plugin manager UI

## Estimated Timeline

- **Wave 1 (4 segments)**: ~12 cycles (~3 hours) - quick wins
- **Wave 2 (4 segments)**: ~27 cycles (~7 hours) - core features
- **Wave 3 (4 segments)**: ~24 cycles (~6 hours) - analytics
- **Wave 4 (3 segments)**: ~22 cycles (~5 hours) - config/perf
- **Wave 5 (4 segments)**: ~22 cycles (~5 hours) - polish

**Total**: ~107 cycles (~26 hours of work)

With 4 concurrent segments in parallel: ~18-22 hours wall-clock time.

## Testing

Each segment must pass:
- `cargo build -p prb-tui` (clean build)
- `cargo nextest run -p prb-tui` (all tests pass)
- `cargo clippy -p prb-tui -- -D warnings` (zero warnings)

Regression tests run on main after merge:
- `cargo nextest run --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Notes

- Segments are independent within waves, parallel execution is safe
- Some segments enable existing code (Wave 1), others add new features (Waves 2-5)
- Multi-tab (S18) is highest complexity at risk 7
- Schema-aware decode (S05) is critical foundation for many later features

## Next Steps

1. ✅ **All segments created** - 19 segments ready for execution
2. **Run orchestrator** - Start with Wave 1 (quick wins)
3. **Review and iterate** - Check results, adjust as needed
4. **Complete all waves** - Work through to completion

### Ready to Execute

```bash
cd ~/probe/.claude/plans/phase3-tui-v2

# Option 1: Execute everything
orchestrate-v2

# Option 2: Start with Wave 1 only (4 quick wins)
orchestrate-v2 --wave 1

# Option 3: Execute specific segment
orchestrate-v2 --segment 01

# Option 4: Dry run to see execution plan
orchestrate-v2 --dry-run
```

All segments are adapted from the original Phase 3 plan, focused on remaining work only.

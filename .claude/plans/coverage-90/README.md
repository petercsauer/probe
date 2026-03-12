# Coverage 90 Plan

**Goal:** Reach 90% workspace test coverage through systematic gap filling

**Baseline:** 61.13% (post-S07 partial from google-level-infra plan)
**Target:** 90%+
**Gap:** ~29 percentage points (~8,000 lines)

## Quick Start

### Execute the Plan

```bash
cd /Users/psauer/probe

# Option 1: Run orchestrator with dashboard
python scripts/orchestrate_v2/orchestrate_v2.py \
  --plan .claude/plans/coverage-90 \
  --monitor

# Option 2: Dry run first
python scripts/orchestrate_v2/orchestrate_v2.py \
  --plan .claude/plans/coverage-90 \
  --dry-run

# Option 3: Execute specific wave
python scripts/orchestrate_v2/orchestrate_v2.py \
  --plan .claude/plans/coverage-90 \
  --wave 1  # Start with quick wins
```

### Check Coverage Manually

```bash
# Full workspace
cargo llvm-cov --workspace --summary-only

# Specific crate
cargo llvm-cov -p prb-grpc --summary-only

# HTML report
cargo llvm-cov --workspace --html
open target/llvm-cov/html/index.html
```

## Plan Structure

- **Manifest:** `manifest.md` - Overview, dependencies, success criteria
- **Config:** `orchestrate.toml` - Orchestrator v2 configuration
- **Segments:** `segments/01-*.md` through `segments/17-*.md`
- **Handoffs:** `handoff/` (generated during execution)

## Wave Breakdown

| Wave | Segments | Theme | Target | Cycles |
|:----:|:--------:|-------|:------:|:------:|
| **1** | S01-S04 | Quick Wins (grpc, pcap, ai, decode) | 61% → 73% | 29 |
| **2** | S05-S07 | Plugin Infrastructure | 73% → 77% | 26 |
| **3** | S08-S09 | CLI Layer | 77% → 80% | 22 |
| **4** | S10-S15 | TUI Deep Dive (6 subsegments!) | 80% → 88% | 67 |
| **5** | S16-S17 | Final Push (errors + gaps) | 88% → 90%+ | 13 |

**Total:** 17 segments, ~137 cycles (~35-40 hours)

## TUI Strategy

prb-tui is split into **6 focused subsegments** (S10-S15):

1. **S10:** app.rs state management (19% → 45%)
2. **S11:** Panes high-value: hex_dump, decode_tree, timeline (→ 75%)
3. **S12:** Panes low-coverage: waterfall, conversations (→ 40%)
4. **S13:** AI features: ai_smart, ai_features (→ 55%)
5. **S14:** Overlays testable: export_dialog, command_palette (→ 50%)
6. **S15:** Config/theme/loader parsing (→ 60%)

**Rationale:** TUI target is 65% (not 85%) because ~40% of code is pure UI rendering (layout, colors, borders) that is better tested via manual QA and snapshot tests.

## Success Criteria

After all 17 segments:

- ✅ Workspace coverage ≥90%
- ✅ All library crates ≥85%
- ✅ prb-tui ≥65% (realistic for UI-heavy code)
- ✅ Critical crates (core, pcap, grpc, decoders) ≥95%
- ✅ All tests pass in CI
- ✅ No regression in existing tests
- ✅ HTML coverage reports generated

## Key Files

```
.claude/plans/coverage-90/
├── README.md                    # This file
├── manifest.md                  # Plan overview and dependencies
├── orchestrate.toml             # Orchestrator v2 config
├── segments/                    # 17 segment definitions
│   ├── 01-grpc-coverage.md
│   ├── 02-pcap-coverage.md
│   ├── 03-ai-coverage.md
│   ├── 04-decode-coverage.md
│   ├── 05-plugin-native-coverage.md
│   ├── 06-plugin-wasm-coverage.md
│   ├── 07-plugin-api-coverage.md
│   ├── 08-cli-coverage.md
│   ├── 09-capture-coverage.md
│   ├── 10-tui-app-state.md
│   ├── 11-tui-panes-high.md
│   ├── 12-tui-panes-low.md
│   ├── 13-tui-ai-features.md
│   ├── 14-tui-overlays.md
│   ├── 15-tui-config-theme.md
│   ├── 16-error-coverage.md
│   └── 17-final-push.md
├── handoff/                     # Generated during execution
└── logs/                        # Execution logs
```

## Dependencies

- **cargo-llvm-cov** - `cargo install cargo-llvm-cov`
- **Python 3.11+** - For orchestrator v2
- **Git** - For worktree isolation
- **Rust toolchain** - nightly or stable with llvm-tools

## Notes

- Orchestrator v2 uses worktree isolation (segments run in parallel)
- Each segment has a cycle budget and auto-retries on failure
- Dashboard available at http://localhost:8082 during execution
- State persisted in `state.db` (safe to resume after interruption)

## Related Plans

- **google-level-infra** - CI/CD, quality gates (completed with S07 partial)
- **S06 handoff** - Coverage analysis baseline (located in google-level-infra plan)

## Questions?

See orchestrator v2 documentation: `scripts/orchestrate_v2/README.md`

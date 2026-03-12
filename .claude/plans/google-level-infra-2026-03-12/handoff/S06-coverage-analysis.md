# Coverage Analysis Results

**Analysis Date:** 2026-03-12
**Tool:** cargo-llvm-cov v0.8.4
**Workspace:** probe (all crates)

## Overall Coverage

**Current:** 61.35%
**Target:** 80.0%
**Gap:** 18.65 percentage points

**Summary Statistics:**
- Total Lines: 43,245
- Covered Lines: 26,532
- Uncovered Lines: 16,713

## Per-Crate Breakdown

### Below Target (<80%) — 11 Crates Need Work

| Crate | Current | Target | Gap | Priority |
|-------|---------|--------|-----|----------|
| **prb-tui** | 38.11% | 80% | -41.89% | **CRITICAL** |
| **prb-plugin-native** | 42.42% | 80% | -37.58% | High |
| **prb-capture** | 45.81% | 80% | -34.19% | High |
| **prb-schema** | 46.41% | 80% | -33.59% | High |
| **prb-cli** | 57.50% | 80% | -22.50% | High |
| prb-fixture | 63.80% | 80% | -16.20% | Medium |
| prb-plugin-wasm | 70.38% | 80% | -9.62% | Medium |
| prb-plugin-api | 74.12% | 80% | -5.88% | Low |
| prb-ai | 77.53% | 80% | -2.47% | Low |
| prb-decode | 78.00% | 80% | -2.00% | Low |
| prb-grpc | 78.52% | 80% | -1.48% | Low |

### Meeting Target (≥80%) — 8 Crates ✅

| Crate | Current | Notes |
|-------|---------|-------|
| **prb-core** | 96.55% | ✅ Excellent (critical foundation) |
| **prb-export** | 96.39% | ✅ Excellent |
| **prb-detect** | 96.03% | ✅ Excellent |
| **prb-query** | 94.22% | ✅ Excellent |
| **prb-dds** | 93.13% | ✅ Excellent (protocol decoder) |
| **prb-zmq** | 92.41% | ✅ Excellent (protocol decoder) |
| prb-storage | 89.08% | ✅ Good |
| prb-pcap | 85.98% | ✅ Good (critical path) |

## Critical Coverage Gaps by File

### prb-tui (38.11% - CRITICAL)
**Issue:** Large TUI implementation with many untested overlay dialogs and panes.

**Major gaps (files with 0% or <25% coverage):**
- `src/overlays/capture_config.rs` - 650 lines, **0% covered** (capture dialog untested)
- `src/overlays/diff_view.rs` - 478 lines, **0% covered** (diff comparison untested)
- `src/overlays/follow_stream.rs` - 383 lines, **0% covered** (stream following untested)
- `src/overlays/theme_editor.rs` - 497 lines, **0% covered** (theme editor untested)
- `src/overlays/tls_keylog_picker.rs` - 301 lines, **0% covered** (TLS keylog picker untested)
- `src/overlays/session_info.rs` - 275 lines, **0% covered** (session info untested)
- `src/overlays/plugin_manager.rs` - 513 lines, **0.97% covered** (plugin manager nearly untested)
- `src/panes/waterfall.rs` - 663 lines, **0.45% covered** (waterfall view nearly untested)
- `src/panes/conversation_list.rs` - 493 lines, **0.61% covered** (conversation list nearly untested)
- `src/app.rs` - 4,339 lines, **19.94% covered** (main app logic largely untested)
- `src/ai_smart.rs` - 570 lines, **20.70% covered** (AI smart features mostly untested)
- `src/loader.rs` - 428 lines, **30.14% covered** (data loading logic undertested)

**Recommendation:** Many TUI features are interactive UI that may be legitimately hard to unit test. Consider:
1. Adding headless/snapshot tests for layout rendering (already exists for some views)
2. Testing business logic separately from rendering
3. Focusing on testing data transformations and state management
4. Some overlay code may be acceptable at lower coverage if it's pure UI

### prb-plugin-native (42.42%)
**Major gaps:**
- `src/loader.rs` - 235 lines, **3.83% covered** (dynamic library loading nearly untested)
- `src/adapter.rs` - 706 lines, **81.02% covered** (adapter tested, but loader critical gap)

**Recommendation:** Plugin loading is error-prone and security-sensitive. Needs comprehensive tests for:
- Valid plugin loading
- Invalid/malformed plugin handling
- Version compatibility checks
- Error paths and failure modes

### prb-capture (45.81%)
**Major gaps:**
- `src/capture.rs` - 163 lines, **8.59% covered** (live capture logic nearly untested)
- `src/adapter.rs` - 132 lines, **39.39% covered** (capture adapter undertested)
- `src/privileges.rs` - 7 lines, **0% covered** (privilege handling untested)

**Recommendation:** Live capture is platform-specific and requires elevated privileges. May need integration tests with mock interfaces.

### prb-schema (46.41%)
**Major gaps:**
- `src/error.rs` - 3 lines, **0% covered** (error type untested - likely thiserror derive)
- Other files have reasonable coverage; low crate average likely due to error type

**Recommendation:** Quick win - add basic error construction tests.

### prb-cli (57.50%)
**Major gaps:**
- `src/commands/tui.rs` - 314 lines, **0% covered** (TUI command entry untested)
- `src/commands/capture.rs` - 194 lines, **18.04% covered** (capture command undertested)

**Recommendation:** CLI command handlers need integration tests. Consider testing:
- Argument parsing
- Validation logic
- Error handling
- Help text generation

### prb-grpc (78.52% - close to target)
**Major gap:**
- `src/h2.rs` - 470 lines, **47.87% covered** (HTTP/2 framing undertested)

**Recommendation:** Quick win to reach 80%. Focus on h2.rs edge cases and error handling.

## Specific Module Gaps

### Error Handling Modules (0% coverage)
Multiple crates have untested error modules (all are thiserror derives):
- `prb-pcap/src/error.rs` - 0%
- `prb-schema/src/error.rs` - 0%
- `prb-plugin-api/src/types.rs` - 0%
- `prb-fixture/src/format.rs` - 0%

**Recommendation:** These are typically thiserror-derived error types. Coverage of 0% is expected for derived code, but consider adding construction/formatting tests.

### Protocol Decoders
- `prb-grpc` (78.52%) - just below target, h2.rs needs work
- `prb-dds` (93.13%) - ✅ excellent coverage
- `prb-zmq` (92.41%) - ✅ excellent coverage

## Recommendations for Segment 07 (Fill Coverage Gaps)

### Phase 1: Critical Foundation (Target: 90%+)
1. ✅ **prb-core** (96.55%) - Already excellent, maintain
2. ✅ **prb-pcap** (85.98%) - Good, but focus on:
   - `src/normalize.rs` (81.47%) - normalization edge cases
   - `src/pipeline.rs` (77.14%) - pipeline error paths
   - `src/tls/decrypt.rs` (82.56%) - TLS decryption edge cases

### Phase 2: High-Value Quick Wins (Close to 80%)
1. **prb-grpc** (78.52% → 80%+) - Focus on `src/h2.rs` (47.87%)
2. **prb-decode** (78.00% → 80%+) - Focus on `src/schema_backed.rs` (69.55%)
3. **prb-ai** (77.53% → 80%+) - Focus on `src/explain.rs` (36.73%)

**Estimated effort:** ~20-30 tests to bring these 3 crates to 80%

### Phase 3: Medium Priority (50-75% coverage)
1. **prb-plugin-api** (74.12%) - Add error construction tests
2. **prb-plugin-wasm** (70.38%) - Focus on loader.rs
3. **prb-fixture** (63.80%) - Test format.rs edge cases
4. **prb-cli** (57.50%) - Add command handler integration tests

### Phase 4: High-Complexity Crates (Consider Scope)
1. **prb-schema** (46.41%) - Investigate actual gaps vs. derived code
2. **prb-capture** (45.81%) - May require integration tests / mocking
3. **prb-plugin-native** (42.42%) - Dynamic loading needs careful testing
4. **prb-tui** (38.11%) - Large UI surface, evaluate ROI on overlay testing

**Note on prb-tui:** 38.11% coverage for a TUI is not necessarily problematic if:
- Core business logic has good coverage (event_store.rs: 96.11%, session.rs: 90.07%)
- Rendering/UI code is tested through snapshot tests (8 snapshot tests exist)
- Interactive features are validated manually

Consider documenting which TUI features are "tested via snapshot" vs. "needs unit tests" vs. "acceptable untested UI".

## Test Effort Estimation

**To reach 80% overall from 61.35%:**
- Need to add ~8,000 lines of coverage
- Estimated: 150-200 new test functions
- Focus areas: CLI commands, plugin loading, TUI business logic, protocol decoder edge cases

**Quick wins (highest ROI):**
1. prb-grpc h2.rs - ~30 tests for 2% crate boost
2. prb-decode schema_backed.rs - ~25 tests for 2% crate boost
3. prb-cli command tests - ~40 tests to reach 80%
4. Error type construction - ~20 tests across multiple crates

## Coverage Report Artifacts

- **HTML Report:** `target/llvm-cov/html/index.html` (not committed)
- **LCOV Data:** `lcov.info` (not committed)
- **Raw Output:** Saved to tool-results directory during analysis

## Next Steps for S07

1. Start with **Phase 2 quick wins** (prb-grpc, prb-decode, prb-ai) to demonstrate momentum
2. Then address **prb-cli** command handlers with integration tests
3. Evaluate **prb-tui** - separate testable business logic from UI rendering
4. Consider **prb-plugin-native** and **prb-capture** as stretch goals requiring more complex test infrastructure

## Success Metrics Met

- ✅ cargo-llvm-cov installed
- ✅ HTML + LCOV reports generated
- ✅ Per-crate coverage calculated
- ✅ Modules below 80% identified and prioritized
- ✅ Coverage gaps documented with specific file/line references
- ✅ Recommendations provided for S07

## Notes

- Overall 61.35% is reasonable for a young codebase
- **prb-core** at 96.55% is excellent - foundation is solid
- Protocol decoders (dds, zmq) have excellent coverage (92-93%)
- Main gaps are in UI (prb-tui), CLI commands, and plugin loading
- Some 0% coverage files are thiserror-derived errors (expected/acceptable)
- TUI coverage may be acceptable given interactive nature and snapshot tests

---
segment: 10
title: "prb-detect + export + zmq + dds + query + misc Coverage"
depends_on: []
risk: 2
complexity: Low
cycle_budget: 4
status: pending
commit_message: "test(workspace): add unit tests for detect, export, zmq, dds, query gaps"
---

# Segment 10: Miscellaneous Coverage Gaps

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Close remaining ≤90% gaps across 6+ crates that each need small amounts of additional coverage.

**Depends on:** None

## Coverage Gaps

| File | Lines | Current | Target | Deficit |
|------|-------|---------|--------|---------|
| prb-zmq/src/correlation.rs | 510 | 75% | 90% | ~78 |
| prb-export/src/har_export.rs | 426 | 80% | 90% | ~42 |
| prb-detect/src/detector.rs | 359 | 86% | 90% | ~15 |
| prb-query/src/ast.rs | 37 | 11% | 90% | ~29 |
| prb-query/src/parser.rs | 471 | 87% | 90% | ~13 |
| prb-export/src/otlp_import.rs | 234 | 83% | 90% | ~15 |
| prb-dds/src/decoder.rs | 597 | 89% | 90% | ~5 |
| prb-dds/src/rtps_parser.rs | 388 | 87% | 90% | ~12 |
| prb-zmq/src/decoder.rs | 459 | 87% | 90% | ~13 |
| prb-detect/src/engine.rs | 126 | 87% | 90% | ~4 |
| prb-detect/src/types.rs | 9 | 33% | 90% | ~5 |
| prb-storage/src/reader.rs | 118 | 84% | 90% | ~7 |

## Scope

Small targeted additions across many crates. Each file needs 5-80 additional covered lines.

## Implementation Approach

### prb-zmq/correlation.rs (75% → 90%)
- Test PUB/SUB correlation with multiple topics
- Test REQ without matching REP (timeout scenario)
- Test DEALER/ROUTER patterns
- Test conversation state transitions

### prb-export/har_export.rs (80% → 90%)
- Test HAR export with HTTPS entries (TLS timing fields)
- Test entries with large payloads (base64 encoding)
- Test empty entries, entries with error responses

### prb-query/ast.rs (11% → 90%)
- Test `Display` impl for all AST node types
- Test `Expr` variants: Binary, Unary, Literal, Field, Function
- Test `Value` Display for each type

### prb-query/parser.rs (87% → 90%)
- Test edge case expressions: nested parentheses, operator precedence
- Test string escaping in filter expressions
- Test error messages for malformed input

### prb-export/otlp_import.rs (83% → 90%)
- Test OTLP JSON import with nested span structures
- Test with missing optional fields

### prb-detect files (86-87% → 90%)
- Test heuristic detection with ambiguous payloads
- Test priority ordering when multiple detectors match
- Test `DetectionResult` Display/Debug

### prb-dds/decoder + rtps_parser (87-89% → 90%)
- Test RTPS submessage types not yet covered (GAP, HEARTBEAT, ACKNACK)
- Test DDS discovery with multiple participants

### prb-storage/reader.rs (84% → 90%)
- Test reading MCAP with multiple channels
- Test seeking and filtering by channel

## Build and Test Commands

- Build: `cargo check --workspace`
- Test (targeted): `cargo nextest run -p prb-zmq -p prb-export -p prb-detect -p prb-query -p prb-dds -p prb-storage`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** All listed crate tests pass
2. **Coverage gate:** Every file listed above ≥ 88% line coverage
3. **Regression tests:** `cargo nextest run --workspace` — no regressions
4. **Full build gate:** `cargo build --workspace`
5. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
6. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
7. **Scope verification gate:** Only test files in listed crates modified

---
segment: 9
title: "prb-decode Codec Tests"
depends_on: []
risk: 3
complexity: Medium
cycle_budget: 4
status: pending
commit_message: "test(prb-decode): add comprehensive schema-backed and wire-format decode tests"
---

# Segment 9: prb-decode Codec Tests

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Bring prb-decode from ~63% to ≥90% line coverage.

**Depends on:** None

## Coverage Gaps

| File | Lines | Current | Target | Deficit |
|------|-------|---------|--------|---------|
| prb-decode/src/schema_backed.rs | 384 | 49% | 90% | ~157 |
| prb-decode/src/wire_format.rs | 508 | 78% | 90% | ~62 |

## Scope

- `crates/prb-decode/src/schema_backed.rs` — Protobuf schema-backed decoder
- `crates/prb-decode/src/wire_format.rs` — Wire-format (schemaless) protobuf decoder

## Implementation Approach

### schema_backed.rs (49% → 90%)
- Test decode with various protobuf field types: varint, fixed32, fixed64, bytes, string, embedded message
- Test nested message decoding
- Test repeated fields and packed repeated fields
- Test oneof fields
- Test enum decode with known and unknown values
- Test error paths: truncated data, wrong wire type, missing required fields
- Test with a real .proto fixture loaded into the schema registry

### wire_format.rs (78% → 90%)
- Test each wire type parser individually
- Test deeply nested messages (recursion limit)
- Test very large varint values
- Test field number extraction
- Test malformed varint (too many continuation bytes)
- Test empty message, single-field message, multi-field message

## Build and Test Commands

- Build: `cargo check -p prb-decode`
- Test (targeted): `cargo nextest run -p prb-decode`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** `cargo nextest run -p prb-decode` — all new tests pass
2. **Coverage gate:** schema_backed.rs ≥ 90%, wire_format.rs ≥ 90%
3. **Regression tests:** `cargo nextest run --workspace` — no regressions
4. **Full build gate:** `cargo build --workspace`
5. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
6. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
7. **Scope verification gate:** Only prb-decode test and source files modified

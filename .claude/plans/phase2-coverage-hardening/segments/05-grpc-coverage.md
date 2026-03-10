---
segment: 5
title: "prb-grpc H2 Parser Tests"
depends_on: []
risk: 3
complexity: Medium
cycle_budget: 4
status: pending
commit_message: "test(prb-grpc): add comprehensive H2 frame and HPACK parser tests"
---

# Segment 5: prb-grpc H2 Parser Tests

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Bring prb-grpc/src/h2.rs from 48% to ≥90% line coverage.

**Depends on:** None

## Coverage Gaps

| File | Lines | Current | Target | Deficit |
|------|-------|---------|--------|---------|
| prb-grpc/src/h2.rs | 473 | 48% | 90% | ~199 |
| prb-grpc/src/decoder.rs | 341 | 80% | 90% | ~32 |

## Scope

- `crates/prb-grpc/src/h2.rs` — HTTP/2 frame parser, HPACK decoder
- `crates/prb-grpc/src/decoder.rs` — gRPC decoder (minor gaps)

## Implementation Approach

### h2.rs (48% → 90%)
This is a pure parser — ideal for unit testing with constructed byte sequences:
- Test each frame type parser: DATA, HEADERS, CONTINUATION, SETTINGS, RST_STREAM, GOAWAY, WINDOW_UPDATE, PING, PRIORITY
- Test HPACK decoder: static table lookups, literal header fields (indexed/not indexed), dynamic table updates, integer decoding (multi-byte)
- Test `parse_integer` with values requiring 1, 2, 3+ bytes
- Test graceful degradation: missing dynamic table context, truncated frames, unknown frame types
- Test HTTP/2 connection preface detection
- Test HEADERS with CONTINUATION chain (multi-frame header blocks)
- Test padding handling on DATA and HEADERS frames
- Test END_STREAM and END_HEADERS flag handling

### decoder.rs (80% → 90%)
- Test gRPC LPM (Length-Prefixed Message) with compressed payloads
- Test grpc-status trailer parsing
- Test error paths: invalid LPM length, missing content-type

## Build and Test Commands

- Build: `cargo check -p prb-grpc`
- Test (targeted): `cargo nextest run -p prb-grpc`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** `cargo nextest run -p prb-grpc` — all new tests pass
2. **Coverage gate:** h2.rs ≥ 90%, decoder.rs ≥ 90%
3. **Regression tests:** `cargo nextest run --workspace` — no regressions
4. **Full build gate:** `cargo build --workspace`
5. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
6. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
7. **Scope verification gate:** Only prb-grpc test and source files modified

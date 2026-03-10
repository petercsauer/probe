---
segment: 7
title: "prb-ai Explain Tests"
depends_on: []
risk: 3
complexity: Low
cycle_budget: 3
status: pending
commit_message: "test(prb-ai): add unit tests for explain engine with mock API client"
---

# Segment 7: prb-ai Explain Tests

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Bring prb-ai from ~65% to ≥90% line coverage by testing the explain engine with mock API responses.

**Depends on:** None

## Coverage Gaps

| File | Lines | Current | Target | Deficit |
|------|-------|---------|--------|---------|
| prb-ai/src/explain.rs | 248 | 36% | 90% | ~133 |
| prb-ai/src/context.rs | 508 | 84% | 90% | ~28 |
| prb-ai/src/config.rs | 162 | 89% | 90% | ~1 |

## Scope

- `crates/prb-ai/src/explain.rs` — ExplainEngine, API call logic
- `crates/prb-ai/src/context.rs` — Context building (minor gap)
- `crates/prb-ai/src/config.rs` — Config validation (1 line gap)

## Implementation Approach

### explain.rs (36% → 90%)
- Create a mock HTTP server or mock the async-openai client trait
- Test `ExplainEngine::explain` with a mocked response → verify text extraction
- Test `explain_streaming` with mocked streaming chunks
- Test error handling: API error, timeout, empty response, malformed response
- Test `build_system_prompt` and `build_user_message` output format
- Test with different event types (gRPC, ZMQ, DDS) to cover protocol-specific prompt paths

### context.rs (84% → 90%)
- Test context building with edge cases: events with no metadata, empty payloads
- Test context window selection with large event lists
- Test `grpc_status_meaning` for all known status codes

### config.rs (89% → 90%)
- Fix the `test_config_from_env` test that fails without OPENAI_API_KEY set (mock the env or skip)

## Build and Test Commands

- Build: `cargo check -p prb-ai`
- Test (targeted): `cargo nextest run -p prb-ai`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** `cargo nextest run -p prb-ai` — all tests pass (including fixing the flaky env test)
2. **Coverage gate:** explain.rs ≥ 90%, context.rs ≥ 90%, config.rs ≥ 90%
3. **Regression tests:** `cargo nextest run --workspace` — no regressions
4. **Full build gate:** `cargo build --workspace`
5. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
6. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
7. **Scope verification gate:** Only prb-ai test and source files modified

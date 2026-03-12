---
segment: 8
title: "prb-plugin Test Harness"
depends_on: []
risk: 6
complexity: High
cycle_budget: 6
status: pending
commit_message: "test(prb-plugin): add test harness with mock plugins for native and WASM loaders"
---

# Segment 8: prb-plugin Test Harness

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Bring prb-plugin-api, prb-plugin-native, and prb-plugin-wasm from ~15% to ≥90% line coverage.

**Depends on:** None

## Coverage Gaps

| File | Lines | Current | Target | Deficit |
|------|-------|---------|--------|---------|
| prb-plugin-native/src/loader.rs | 236 | 0% | 90% | ~212 |
| prb-plugin-wasm/src/adapter.rs | 229 | 0% | 90% | ~206 |
| prb-plugin-native/src/adapter.rs | 277 | 34% | 90% | ~156 |
| prb-plugin-wasm/src/loader.rs | 126 | 23% | 90% | ~84 |
| prb-plugin-api/src/native.rs | 36 | 0% | 90% | ~32 |
| prb-plugin-wasm/src/runtime.rs | 9 | 0% | 90% | ~8 |
| prb-plugin-api/src/types.rs | 4 | 0% | 90% | ~3 |

## Scope

- `crates/prb-plugin-api/src/` — Plugin API types, native FFI definitions
- `crates/prb-plugin-native/src/` — Native .so/.dylib plugin loader and adapter
- `crates/prb-plugin-wasm/src/` — WASM/Extism plugin loader and adapter

## Implementation Approach

### Strategy: Build minimal test plugins
The key challenge is that loaders need actual plugin binaries. Two approaches:

**Native plugins:**
- Create a minimal test shared library in-tree (a small .rs file compiled as cdylib)
- Or: mock the `libloading::Library` at the trait boundary — wrap loading in a trait and provide a mock
- Test symbol resolution, version checking, error paths (missing symbols, wrong version)

**WASM plugins:**
- If a test .wasm exists, use it. If not, create a minimal WASM module via a build script or check in a tiny .wasm fixture
- Or: test the adapter conversion logic separately (DecodeContext → DecodeCtx, DTO → DebugEvent) without loading real WASM
- Test error paths: corrupt WASM, missing functions, version mismatch

### plugin-api types (0%)
- Test DTO serialization/deserialization round-trips
- Test native FFI struct layout assertions
- Test `PluginInfo` Display impl

### native/adapter.rs (34% → 90%)
- Test `NativeDecoderFactory::decode` with mock function pointers
- Test `NativeProtocolDetector::detect` with synthetic data
- Test error conversion from C error codes

### native/loader.rs (0% → 90%)
- If building a test cdylib: test full load → info → detect → decode cycle
- If mocking: test each validation step (symbol lookup, version check, info parsing)
- Test error paths exhaustively

### wasm/adapter.rs (0% → 90%)
- Test `convert_decode_context` with various input combinations
- Test `convert_dto_to_event` with all direction/transport/protocol variants
- Test metadata map conversion

### wasm/loader.rs (23% → 90%)
- Test WASM plugin loading with a fixture
- Test version validation
- Test error paths: missing file, invalid WASM, missing exports

## Pre-Mortem Risks

- Building test plugins (cdylib/WASM) adds build complexity — keep them minimal
- libloading tests may be platform-specific — use cfg attributes for platform differences
- If real plugin binaries are too complex, focus on testing conversion/adaptation logic (which is most of the code) and accept that the actual dlopen/extism_plugin_new paths may need integration tests

## Build and Test Commands

- Build: `cargo check -p prb-plugin-api -p prb-plugin-native -p prb-plugin-wasm`
- Test (targeted): `cargo nextest run -p prb-plugin-api -p prb-plugin-native -p prb-plugin-wasm`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** All plugin crate tests pass
2. **Coverage gate:** adapter.rs files ≥ 85%, loader.rs files ≥ 80%, API types ≥ 90%
3. **Regression tests:** `cargo nextest run --workspace` — no regressions
4. **Full build gate:** `cargo build --workspace`
5. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
6. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
7. **Scope verification gate:** Only prb-plugin-* test and source files modified

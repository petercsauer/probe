---
id: "8"
title: "prb-plugin-wasm loader requires real WASM module"
risk: 6/10
addressed_by_segments: [8]
---

# Issue 8: prb-plugin-wasm loader requires real WASM module

## Core Problem

`loader.rs` (35% coverage) and `adapter.rs` (69%) lack integration tests because `WasmPluginLoader::load()` requires a valid `.wasm` file with Extism-compatible exports (`prb_plugin_info`, `prb_plugin_detect`, `prb_plugin_decode`). `validate_exports()`, `load_directory()`, and all adapter FFI paths are untested.

## Root Cause

No WASM test fixture exists. Building Rust to `wasm32-unknown-unknown` with Extism PDK is a separate toolchain concern.

## Proposed Fix

1. Create `crates/prb-plugin-wasm/tests/fixtures/test-plugin/` as a Rust crate targeting `wasm32-unknown-unknown`.
2. Use `extism-pdk` to implement the three required exports with trivial responses.
3. Pre-build the `.wasm` and check it into `tests/fixtures/test_plugin.wasm` for CI portability.
4. Test: `load()` success + metadata, `load()` with random bytes (invalid WASM), `load_directory()` empty/mixed, `validate_exports()` with incomplete module, `detect()`, `decode()`.

## Existing Solutions Evaluated

- **extism-pdk** (crates.io): Official Extism plugin SDK for Rust→WASM. Required.
- Alternative: hand-write WAT (WebAssembly Text) with correct exports. Simpler but fragile for JSON I/O. Rejected.

## Pre-Mortem

- `wasm32-unknown-unknown` target must be installed: `rustup target add wasm32-unknown-unknown`.
- Extism PDK version must match the host `extism` crate version (1.10).
- Pre-built `.wasm` binary checked into git adds ~50KB. Acceptable.
- CI must have the wasm target or the test must be `#[ignore]` with a feature gate.

## Risk Factor: 6/10

Cross-compilation toolchain, Extism version coupling.

## Blast Radius

- Direct: new `tests/fixtures/` crate or pre-built wasm, `crates/prb-plugin-wasm/tests/loader_test.rs`
- Ripple: `Cargo.toml` workspace (if adding fixture crate), CI config (wasm target)

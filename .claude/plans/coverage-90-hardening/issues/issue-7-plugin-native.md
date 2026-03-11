---
id: "7"
title: "prb-plugin-native loader requires real shared library"
risk: 6/10
addressed_by_segments: [7]
---

# Issue 7: prb-plugin-native loader requires real shared library

## Core Problem

`loader.rs` (6% coverage, 276 lines) is nearly untested because `NativePluginLoader::load()` requires a real `.dylib`/`.so` exporting 6 specific C ABI symbols. `adapter.rs` (75%) has full coverage on `dto_to_debug_event` but zero coverage on `NativeDecoderFactory`, `NativeDecoderInstance`, and `NativeProtocolDetector` — all of which need a `LoadedPlugin` from the loader.

## Root Cause

No test plugin fixture exists. Building a cdylib is non-trivial to set up as a test dependency.

## Proposed Fix

1. Create `crates/prb-plugin-native/tests/fixtures/test-plugin/` as a minimal Rust crate with `crate-type = ["cdylib"]`.
2. Implement `PluginDecoder` trait from `prb-plugin-api` using `prb_export_plugin!(TestDecoder)` macro.
3. `TestDecoder::detect` returns confidence 0.9 for any input. `TestDecoder::decode` returns a single `DebugEventDto` with fixed fields.
4. Build fixture as part of `cargo test` using a `build.rs` or `#[ctor]` setup.
5. Test: `load()` success, `load()` with invalid binary, `load_directory()` with empty/mixed dirs, `detect()`, `create_decoder()`, `decode()`, `destroy_decoder()`.

## Existing Solutions Evaluated

- `prb-plugin-api/tests/native_test.rs` already has `TestDecoder` that implements the trait — this is the template.
- `libloading` crate docs show test patterns with cdylib fixtures.

## Pre-Mortem

- Cross-compilation: the cdylib must be built for the host target. CI may need explicit `--target` flag.
- `build.rs` approach adds build complexity. Alternative: use `cargo build -p test-plugin` in test setup.
- Library path resolution: tests must locate the built `.dylib` — use `env!("CARGO_TARGET_DIR")` or `target/debug/` convention.

## Risk Factor: 6/10

Build system complexity, cross-platform cdylib naming (.so vs .dylib vs .dll).

## Blast Radius

- Direct: new `tests/fixtures/test-plugin/` crate, `crates/prb-plugin-native/tests/loader_test.rs`
- Ripple: `Cargo.toml` workspace members list (if adding fixture crate)

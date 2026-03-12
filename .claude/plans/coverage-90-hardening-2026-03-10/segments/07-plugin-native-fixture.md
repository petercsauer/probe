---
segment: 7
title: "Plugin-native test fixture"
depends_on: []
risk: 6
complexity: High
cycle_budget: 20
status: pending
commit_message: "test(prb-plugin-native): add cdylib test fixture and loader/adapter integration tests"
---

# Segment 7: Plugin-native test fixture

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Push prb-plugin-native from 55.6% to 85%+ by building a minimal cdylib test plugin and writing loader/adapter integration tests.

**Depends on:** None

## Issues Addressed

Issue 7 — prb-plugin-native loader requires real shared library.

## Scope

- New crate: `crates/prb-plugin-native/tests/fixtures/test-plugin/` (or workspace member)
- `crates/prb-plugin-native/tests/loader_test.rs` — new integration tests
- `crates/prb-plugin-native/tests/adapter_test.rs` — new adapter integration tests

## Key Files and Context

**Required C ABI exports (from prb-plugin-api/src/native.rs):**
```rust
prb_plugin_info()           -> PluginInfo { name, version, description, api_version, protocol_id }
prb_plugin_detect(buf, src_port, dst_port, transport) -> DetectResultFfi { detected, confidence }
prb_plugin_decoder_create() -> *mut c_void
prb_plugin_decode(decoder, data_buf, context_buf) -> OwnedBuffer
prb_plugin_buffer_free(buf: OwnedBuffer)
prb_plugin_decoder_destroy(decoder: *mut c_void)
```

**prb-plugin-api provides `prb_export_plugin!` macro** that generates all these exports from a `PluginDecoder` trait impl. See `prb-plugin-api/tests/native_test.rs` lines ~60-90 for a working `TestDecoder` implementation.

**`NativePluginLoader::load()` (loader.rs ~97-215):**
1. `Library::new(path)` — loads .dylib/.so
2. Gets each of the 6 symbols
3. Calls `prb_plugin_info()` to get metadata
4. Reads string fields via `CStr::from_ptr`
5. Calls `validate_api_version`
6. Stores `LoadedPlugin`

**Platform-specific extension (loader.rs `load_directory` ~224-261):**
- macOS: `.dylib`
- Linux: `.so`
- Windows: `.dll`

## Implementation Approach

1. Create `crates/prb-plugin-native-test-fixture/` as a workspace member:
   ```toml
   [package]
   name = "prb-plugin-native-test-fixture"
   edition = "2024"
   publish = false
   
   [lib]
   crate-type = ["cdylib"]
   
   [dependencies]
   prb-plugin-api = { path = "../prb-plugin-api" }
   serde_json = "1"
   ```

2. Implement minimal `TestDecoder`:
   ```rust
   use prb_plugin_api::native::*;
   
   struct TestNativeDecoder;
   
   impl PluginDecoder for TestNativeDecoder {
       fn info() -> PluginMetadata { /* name: "test-native", protocol: "test", version: "0.1.0" */ }
       fn detect(ctx: &DetectContext) -> Option<f32> { Some(0.9) }
       fn create() -> Self { TestNativeDecoder }
       fn decode(&mut self, data: &[u8], ctx: &[u8]) -> Vec<u8> { /* return JSON DebugEventDto */ }
   }
   
   prb_export_plugin!(TestNativeDecoder);
   ```

3. Add to workspace `Cargo.toml` members list.

4. In `crates/prb-plugin-native/tests/loader_test.rs`:
   - Locate built fixture: `env!("CARGO_TARGET_DIR")` or `target/debug/libprb_plugin_native_test_fixture.dylib`
   - `test_load_valid_plugin`: `loader.load(fixture_path)`, assert metadata fields
   - `test_load_invalid_binary`: write `b"not a library"` to temp `.dylib`, assert `PluginError::Load`
   - `test_load_directory_empty`: empty temp dir, assert empty result
   - `test_load_directory_with_plugin`: copy fixture to temp dir, assert loaded
   - `test_load_directory_skips_non_lib`: dir with `.txt` files, assert empty

5. In adapter tests:
   - `test_detect_with_loaded_plugin`: call `detect()` on loaded plugin, assert confidence
   - `test_create_and_destroy_decoder`: create decoder, decode some bytes, destroy
   - `test_protocol_detector`: construct `NativeProtocolDetector` from loaded plugin, call `detect()`

## Alternatives Ruled Out

- Mock `Library` type via trait: too complex, doesn't test real FFI.
- Pre-built binary checked into git: brittle across platforms. Build from source.
- WAT/C plugin: Rust cdylib using the existing macro is simpler and tests the real API.

## Pre-Mortem Risks

- **Library naming:** macOS=`libX.dylib`, Linux=`libX.so`. Use `std::env::consts::DLL_PREFIX` and `DLL_SUFFIX`.
- **Build ordering:** `cargo test -p prb-plugin-native` must build the fixture first. Adding it as a workspace member ensures this. Alternatively, use a `build-dependency` or manual `cargo build -p prb-plugin-native-test-fixture` in test setup.
- **Symbol visibility:** `prb_export_plugin!` must produce `#[no_mangle] pub extern "C"` symbols. Verify with `nm -gD` on the built library.
- **Cargo workspace:** Adding a new workspace member changes `Cargo.lock` and may affect CI.

## Build and Test Commands

- Build fixture: `cargo build -p prb-plugin-native-test-fixture`
- Build: `cargo build -p prb-plugin-native`
- Test (targeted): `cargo test -p prb-plugin-native -- loader && cargo test -p prb-plugin-native -- adapter`
- Test (regression): `cargo test -p prb-plugin-native`
- Test (full gate): `cargo test -p prb-plugin-native`

## Exit Criteria

1. **Targeted tests:**
   - `test_load_valid_plugin`: metadata name/version/protocol match fixture values
   - `test_load_invalid_binary`: returns PluginError::Load
   - `test_load_directory_empty`: empty vec
   - `test_load_directory_with_plugin`: one plugin loaded
   - `test_detect_with_loaded_plugin`: returns Some(confidence)
   - `test_create_and_destroy_decoder`: no panic
2. **Regression tests:** All existing prb-plugin-native tests pass (21 adapter tests)
3. **Full build gate:** `cargo build -p prb-plugin-native -p prb-plugin-native-test-fixture`
4. **Full test gate:** `cargo test -p prb-plugin-native`
5. **Self-review gate:** Fixture crate is minimal, no unnecessary deps
6. **Scope verification gate:** New fixture crate + prb-plugin-native test files only

**Risk factor:** 6/10
**Estimated complexity:** High

---
segment: 8
title: "Plugin-wasm test fixture"
depends_on: []
risk: 6
complexity: High
cycle_budget: 20
status: pending
commit_message: "test(prb-plugin-wasm): add WASM test fixture and loader/adapter integration tests"
---

# Segment 8: Plugin-wasm test fixture

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Push prb-plugin-wasm from 66.1% to 85%+ by building a minimal WASM test plugin and writing loader/adapter integration tests.

**Depends on:** None

## Issues Addressed

Issue 8 — prb-plugin-wasm loader requires real WASM module.

## Scope

- New crate: `crates/prb-plugin-wasm-test-fixture/` targeting `wasm32-unknown-unknown`
- Pre-built `.wasm` checked into `crates/prb-plugin-wasm/tests/fixtures/`
- `crates/prb-plugin-wasm/tests/loader_test.rs` — new integration tests
- `crates/prb-plugin-wasm/tests/adapter_test.rs` — new adapter tests

## Key Files and Context

**Required Extism exports (JSON-based):**
```
prb_plugin_info()               -> String (JSON PluginMetadata)
prb_plugin_detect(String)       -> String (JSON Option<f32>)
prb_plugin_decode(String)       -> String (JSON Vec<DebugEventDto>)
```

**PluginMetadata format (from prb-plugin-api/src/types.rs):**
```json
{
  "name": "test-wasm",
  "version": "0.1.0",
  "api_version": "0.1.0",
  "protocol_id": "test",
  "description": "Test WASM plugin",
  "transport": "Tcp"
}
```

**WasmPluginLoader::load() (loader.rs ~38-87):**
1. `Manifest::new([Wasm::file(path)])` — creates Extism manifest
2. `Plugin::new(manifest, [], true)` — instantiates WASM
3. `validate_exports(&plugin)` — checks all 3 exports exist
4. `plugin.call::<&str, &str>("prb_plugin_info", "")` — gets metadata JSON
5. `serde_json::from_str::<PluginMetadata>` — deserializes
6. `validate_api_version` — checks compatibility

**validate_exports (loader.rs ~118-128):** Checks plugin has functions `prb_plugin_info`, `prb_plugin_detect`, `prb_plugin_decode`.

## Implementation Approach

1. Create `crates/prb-plugin-wasm-test-fixture/`:
   ```toml
   [package]
   name = "prb-plugin-wasm-test-fixture"
   edition = "2024"
   publish = false
   
   [lib]
   crate-type = ["cdylib"]
   
   [dependencies]
   extism-pdk = "1"
   serde = { version = "1", features = ["derive"] }
   serde_json = "1"
   ```

2. Implement exports using `extism-pdk`:
   ```rust
   use extism_pdk::*;
   
   #[plugin_fn]
   pub fn prb_plugin_info(_: String) -> FnResult<String> {
       Ok(r#"{"name":"test-wasm","version":"0.1.0","api_version":"0.1.0","protocol_id":"test","description":"Test WASM plugin","transport":"Tcp"}"#.to_string())
   }
   
   #[plugin_fn]
   pub fn prb_plugin_detect(_input: String) -> FnResult<String> {
       Ok("0.9".to_string())
   }
   
   #[plugin_fn]
   pub fn prb_plugin_decode(_input: String) -> FnResult<String> {
       Ok("[]".to_string())
   }
   ```

3. Build and check in the `.wasm`:
   ```bash
   rustup target add wasm32-unknown-unknown
   cargo build -p prb-plugin-wasm-test-fixture --target wasm32-unknown-unknown --release
   cp target/wasm32-unknown-unknown/release/prb_plugin_wasm_test_fixture.wasm \
      crates/prb-plugin-wasm/tests/fixtures/test_plugin.wasm
   ```

4. Add the fixture crate to workspace (exclude from default members if needed).

5. Write tests in `crates/prb-plugin-wasm/tests/loader_test.rs`:
   - `test_load_valid_wasm`: load fixture, assert metadata fields
   - `test_load_invalid_wasm`: write random bytes to `.wasm`, assert error
   - `test_load_directory_empty`: empty dir, empty result
   - `test_load_directory_with_wasm`: dir containing fixture, one plugin loaded
   - `test_validate_exports_missing`: load a `.wasm` without the required exports (use a minimal WAT or empty WASM)
   - `test_detect_with_wasm_plugin`: call detect, assert Some(0.9)
   - `test_decode_with_wasm_plugin`: call decode, assert empty vec

## Alternatives Ruled Out

- Hand-written WAT: fragile for JSON string I/O, hard to maintain. Rejected.
- Mock `Plugin` type: Extism doesn't expose a test double. Rejected.
- Download fixture from CI artifact: adds network dependency to tests. Rejected.

## Pre-Mortem Risks

- **wasm32-unknown-unknown target:** Must be installed. Add `rustup target add wasm32-unknown-unknown` to CI.
- **extism-pdk version:** Must match host `extism` crate version (1.10). Pin `extism-pdk = "1"` to stay in range.
- **Pre-built binary size:** ~50-100KB `.wasm` checked into git. Acceptable.
- **Workspace changes:** Adding a WASM-target crate to workspace may confuse `cargo test --workspace` on hosts without the WASM target. Solution: use `[workspace.exclude]` or `default-members` to exclude the fixture from workspace-wide builds. Only build it explicitly.

## Build and Test Commands

- Build fixture: `cargo build -p prb-plugin-wasm-test-fixture --target wasm32-unknown-unknown --release`
- Build: `cargo build -p prb-plugin-wasm`
- Test (targeted): `cargo test -p prb-plugin-wasm -- loader && cargo test -p prb-plugin-wasm -- adapter`
- Test (regression): `cargo test -p prb-plugin-wasm`
- Test (full gate): `cargo test -p prb-plugin-wasm`

## Exit Criteria

1. **Targeted tests:**
   - `test_load_valid_wasm`: metadata matches fixture values
   - `test_load_invalid_wasm`: returns error
   - `test_load_directory_empty`: empty vec
   - `test_load_directory_with_wasm`: one plugin loaded
   - `test_detect_with_wasm_plugin`: returns Some(confidence)
   - `test_decode_with_wasm_plugin`: returns empty events vec
2. **Regression tests:** All existing prb-plugin-wasm tests pass (22 tests)
3. **Full build gate:** `cargo build -p prb-plugin-wasm`
4. **Full test gate:** `cargo test -p prb-plugin-wasm`
5. **Self-review gate:** Pre-built .wasm present, fixture crate is minimal
6. **Scope verification gate:** New fixture crate + prb-plugin-wasm test files only

**Risk factor:** 6/10
**Estimated complexity:** High

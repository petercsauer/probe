---
segment: 2
title: "Exclude WASM Fixture from Workspace"
depends_on: []
risk: 2/10
complexity: Low
cycle_budget: 10
status: pending
commit_message: "fix(build): Exclude WASM test fixture from workspace builds"
---

# Segment 2: Exclude WASM Fixture from Workspace

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Exclude prb-plugin-wasm-test-fixture from workspace builds to fix native linking failures.

**Depends on:** None

## Context: Issues Addressed

**Core Problem:** The `prb-plugin-wasm-test-fixture` crate is compiled as a cdylib for wasm32-unknown-unknown target using `extism-pdk`. When CI runs `cargo build --workspace`, it attempts native compilation for aarch64/x86_64, causing linker failures for undefined WASM runtime symbols: `_alloc`, `_error_set`, `_input_length`, `_input_load_u64`, `_input_load_u8`, `_output_set`, `_store_u64`, `_store_u8`. These symbols are provided by the Extism WASM runtime and only exist in WASM sandbox environments. The error message is:

```
error: linking with `cc` failed: exit status: 1
Undefined symbols for architecture arm64:
  "_alloc", referenced from: extism_pdk::memory::Memory::new::h03eeee5f0a832e7e
  "_error_set", referenced from: _prb_plugin_decode, _prb_plugin_detect, _prb_plugin_info
  ...
ld: symbol(s) not found for architecture arm64
```

**Proposed Fix:** Add `crates/prb-plugin-wasm-test-fixture` to workspace `exclude` list in root `Cargo.toml`. The fixture generates a pre-built binary at `crates/prb-plugin-wasm/tests/fixtures/test_plugin.wasm` (already checked into git) used by 40 integration tests. The fixture source code rarely needs rebuilding and should only be compiled manually when the fixture implementation changes.

**Pre-Mortem Risks:**
- Developers won't know how to rebuild fixture if modified (add clear README with build instructions)
- Excluded crate won't get automatic dependency updates (acceptable - fixture is simple and changes infrequently)
- Someone might re-add to members thinking exclusion was accidental (add comment explaining why excluded)
- Fixture could drift from extism-pdk versions (acceptable - test failures would catch incompatibility)

## Scope

- `/Users/psauer/probe/Cargo.toml` - Workspace root configuration
- `/Users/psauer/probe/crates/prb-plugin-wasm-test-fixture/README.md` - New documentation file

## Key Files and Context

**Fixture crate** (`/Users/psauer/probe/crates/prb-plugin-wasm-test-fixture/`):
- `Cargo.toml`: Specifies `crate-type = ["cdylib"]` and depends on `extism-pdk = "1"` for WASM plugin development
- `src/lib.rs` (67 lines): Implements 3 exports using `#[plugin_fn]` macro:
  - `prb_plugin_info()` - Returns JSON plugin metadata (name, version, description)
  - `prb_plugin_detect(input: &[u8])` - Protocol detection logic (returns boolean + confidence score)
  - `prb_plugin_decode(input: &[u8])` - Decoding logic (returns decoded protocol structure)
- **Purpose:** Generate test fixture for integration tests, NOT used at runtime in production

**Pre-built binary** (`/Users/psauer/probe/crates/prb-plugin-wasm/tests/fixtures/test_plugin.wasm`):
- Already exists and is checked into git (committed March 12, 2026)
- Size: ~100KB compiled WASM module
- Used by tests in:
  - `crates/prb-plugin-wasm/tests/loader_test.rs` (17 tests) - Tests plugin loading, initialization, error handling
  - `crates/prb-plugin-wasm/tests/adapter_test.rs` (23 tests) - Tests protocol detection, decoding, metadata extraction
- Tests load via: `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test_plugin.wasm")`
- Tests do NOT reference the source crate, only the compiled .wasm file

**Workspace root** (`/Users/psauer/probe/Cargo.toml`):
```toml
[workspace]
resolver = "3"
members = [
    "crates/*",  # Glob pattern includes prb-plugin-wasm-test-fixture
]
exclude = [
    "fuzz",  # Already excluded for similar reasons (fuzz targets are target-specific)
]
```

**Integration tests using fixture:**
- `/Users/psauer/probe/crates/prb-plugin-wasm/tests/loader_test.rs` - 17 tests covering:
  - Plugin loading from file path
  - Plugin initialization and configuration
  - Function calling (info, detect, decode)
  - Error handling for invalid plugins
  - Memory management and cleanup
- `/Users/psauer/probe/crates/prb-plugin-wasm/tests/adapter_test.rs` - 23 tests covering:
  - WasmDecoderFactory creation
  - Protocol detection with confidence scores
  - Decoding protocol messages
  - Metadata extraction (plugin name, version, supported protocols)
  - Adapter lifecycle (create, use, drop)

## Implementation Approach

1. **Update workspace exclusion:**

   Edit `/Users/psauer/probe/Cargo.toml`, modify the `exclude` array:
   ```toml
   [workspace]
   resolver = "3"
   members = ["crates/*"]
   exclude = [
       "fuzz",
       "crates/prb-plugin-wasm-test-fixture",  # WASM-only build, excluded from native builds
   ]
   ```

2. **Add comprehensive README documentation:**

   Create `/Users/psauer/probe/crates/prb-plugin-wasm-test-fixture/README.md`:
   ```markdown
   # WASM Plugin Test Fixture

   This crate generates a minimal WASM plugin for testing `prb-plugin-wasm`.

   ## Why Excluded from Workspace?

   This crate **must be compiled to `wasm32-unknown-unknown`** because it uses `extism-pdk`,
   which provides WASM-only runtime imports (`_alloc`, `_error_set`, `_input_*`, `_output_set`, etc.).

   Building for native targets (x86_64, aarch64) causes linker errors:
   ```
   Undefined symbols for architecture arm64:
     "_alloc", "_error_set", "_input_length", ...
   ```

   These symbols are provided by the Extism WASM runtime and only exist when the code
   runs inside a WASM sandbox.

   ## Pre-Built Binary

   The pre-built WASM binary is checked into git at:
   ```
   crates/prb-plugin-wasm/tests/fixtures/test_plugin.wasm
   ```

   This binary is used by 40 integration tests in:
   - `crates/prb-plugin-wasm/tests/loader_test.rs` (17 tests)
   - `crates/prb-plugin-wasm/tests/adapter_test.rs` (23 tests)

   ## When to Rebuild

   You only need to rebuild the fixture if you modify:
   - The plugin implementation in `src/lib.rs`
   - The `extism-pdk` version in `Cargo.toml`
   - The plugin exports (info, detect, decode functions)

   Most development work does NOT require rebuilding the fixture.

   ## How to Rebuild

   ### One-Time Setup

   Install the WASM target (only needed once per machine):
   ```bash
   rustup target add wasm32-unknown-unknown
   ```

   ### Build Process

   ```bash
   # Build the fixture (from repo root)
   cargo build -p prb-plugin-wasm-test-fixture \
       --target wasm32-unknown-unknown \
       --release

   # Copy to test fixtures directory
   cp target/wasm32-unknown-unknown/release/prb_plugin_wasm_test_fixture.wasm \
      crates/prb-plugin-wasm/tests/fixtures/test_plugin.wasm

   # Verify tests still pass
   cargo test -p prb-plugin-wasm

   # Commit the updated binary
   git add crates/prb-plugin-wasm/tests/fixtures/test_plugin.wasm
   git commit -m "test(plugin): Update WASM test fixture"
   ```

   ### Verification

   After rebuilding, verify the integration tests pass:
   ```bash
   cargo test -p prb-plugin-wasm loader_test
   cargo test -p prb-plugin-wasm adapter_test
   ```

   ## Fixture Implementation

   The fixture implements the three required Extism exports:

   1. **`prb_plugin_info()`** - Returns plugin metadata as JSON:
      ```json
      {
        "name": "test-plugin",
        "version": "0.1.0",
        "description": "Test plugin for integration tests"
      }
      ```

   2. **`prb_plugin_detect(input: &[u8])`** - Protocol detection:
      - Returns `true` if input matches test protocol pattern
      - Returns confidence score (0.0-1.0)

   3. **`prb_plugin_decode(input: &[u8])`** - Decoding:
      - Parses input according to test protocol
      - Returns decoded structure as JSON

   ## Dependencies

   - `extism-pdk = "1"` - Extism Plugin Development Kit
   - Provides `#[plugin_fn]` macro for exports
   - Provides memory management utilities for WASM
   ```

3. **Verify builds work without fixture:**
   ```bash
   cargo build --workspace
   # Should succeed without trying to build the test fixture
   # Should build all 24 other workspace crates successfully
   ```

4. **Verify tests still pass with pre-built binary:**
   ```bash
   cargo test -p prb-plugin-wasm
   # Should run all 40 integration tests successfully
   # Tests load the pre-built test_plugin.wasm file
   ```

5. **Verify pre-built binary is intact:**
   ```bash
   ls -lh crates/prb-plugin-wasm/tests/fixtures/test_plugin.wasm
   # Should exist and be approximately 100KB in size
   file crates/prb-plugin-wasm/tests/fixtures/test_plugin.wasm
   # Should report: "WebAssembly (wasm) binary module"
   ```

6. **Test a manual rebuild (optional verification):**
   ```bash
   # Ensure wasm32 target is installed
   rustup target add wasm32-unknown-unknown

   # Build the fixture manually
   cargo build -p prb-plugin-wasm-test-fixture \
       --target wasm32-unknown-unknown \
       --release

   # Verify the build output exists
   ls -lh target/wasm32-unknown-unknown/release/prb_plugin_wasm_test_fixture.wasm
   ```

## Alternatives Ruled Out

- **Use `default-members` instead of `exclude`:** Rejected - requires listing all 23 other crates vs excluding 1, high maintenance burden when adding new crates
- **Add CI step to build WASM fixture:** Rejected - adds 30+ seconds to every CI run when pre-built binary already works and changes infrequently
- **Remove fixture source entirely, keep only binary:** Rejected - loses ability to rebuild fixture if implementation needs changes or debugging
- **Create separate workspace for WASM crates:** Rejected - overkill for a single small test fixture, adds complexity to repo structure
- **Conditional compilation with cargo features:** Rejected - would require complex cfg attributes and target-specific dependencies, over-engineered for this use case

## Pre-Mortem Risks

- **Developers modify fixture without knowing how to rebuild:** Mitigation - Clear, comprehensive README with step-by-step instructions and one-time setup guide
- **Fixture gets out of sync with extism-pdk versions:** Mitigation - Infrequent changes expected, test failures would immediately catch incompatibility
- **README instructions become outdated:** Mitigation - Test rebuild process before documenting, include verification steps in README
- **Confusion about why crate is excluded:** Mitigation - Inline comment in Cargo.toml explaining exclusion, detailed README rationale section

## Build and Test Commands

- Build: `cargo build --workspace` (should succeed without fixture)
- Test (targeted): `cargo test -p prb-plugin-wasm` (uses pre-built .wasm, all 40 tests)
- Test (regression): `cargo test --workspace`
- Test (full gate): `cargo nextest run --workspace`
- Verify exclusion: `cargo metadata --no-deps | jq '.workspace_members' | grep -v test-fixture`

## Exit Criteria

1. **Targeted tests:**
   - `cargo build --workspace` succeeds without linking errors
   - `cargo test -p prb-plugin-wasm` passes all 40 integration tests
   - `cargo test -p prb-plugin-wasm loader_test` passes (17 tests)
   - `cargo test -p prb-plugin-wasm adapter_test` passes (23 tests)
   - Pre-built binary exists: `ls crates/prb-plugin-wasm/tests/fixtures/test_plugin.wasm`

2. **Regression tests:**
   - All workspace tests pass: `cargo test --workspace`
   - All prb-plugin-wasm tests specifically pass (verify WASM integration)
   - No other packages affected by exclusion change

3. **Full build gate:**
   - `cargo build --workspace --all-targets` succeeds (no test fixture in build)
   - `cargo clippy --workspace --all-targets -- -D warnings` passes

4. **Full test gate:**
   - `cargo nextest run --workspace` passes all workspace tests

5. **Self-review gate:**
   - README is comprehensive with clear instructions
   - README includes troubleshooting section
   - Workspace exclusion documented with inline comment
   - Rebuild process tested and verified before documenting
   - No production code changes (only Cargo.toml and new README)

6. **Scope verification gate:**
   - Modified files:
     - `Cargo.toml` (workspace root - add 1 line to exclude array)
     - `crates/prb-plugin-wasm-test-fixture/README.md` (new file, comprehensive documentation)
   - No changes to:
     - Test fixture source code (src/lib.rs, Cargo.toml in fixture)
     - Pre-built binary (test_plugin.wasm)
     - Any other workspace crates
     - CI configuration files

**Risk factor:** 2/10

**Estimated complexity:** Low

**Commit message:** `fix(build): Exclude WASM test fixture from workspace builds`

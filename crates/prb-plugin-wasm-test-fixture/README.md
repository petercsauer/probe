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

<!-- cargo-rdme -->

---
issue: 3
title: "ABI stability for native plugins"
severity: High
segments_affected: [4]
status: open
---

# Issue 3: ABI Stability for Native Plugins

## Problem

Native plugins (shared libraries) are compiled separately from the host. If the
plugin ABI changes between versions, a plugin compiled against API v0.1 may crash
or corrupt memory when loaded by a host expecting API v0.2.

Rust does not have a stable ABI — `repr(Rust)` layouts can change between compiler
versions. We use `repr(C)` and `extern "C"` to get a stable C ABI, but this only
covers the function signatures and struct layouts we explicitly define.

## Specific Risks

### R1: Struct layout changes
If `PluginInfo`, `DetectResultFfi`, `ByteBuffer`, or `OwnedBuffer` gain or lose
fields, old plugins will pass wrong-sized data.

### R2: Function signature changes
Adding a parameter to `prb_plugin_decode()` breaks all existing plugins.

### R3: Semantic changes
Changing the meaning of a field (e.g., `confidence` from 0-1 to 0-100) silently
breaks behavior without a compilation error.

### R4: Rust compiler version mismatch
Even with `repr(C)`, if the plugin and host use different allocators (jemalloc vs
system), `OwnedBuffer` freeing can crash.

## Mitigation

### M1: Strict semver on `prb-plugin-api`

The `API_VERSION` constant follows semver:
- **Major** bump: Breaking changes (struct layout, function signature). Old
  plugins are rejected.
- **Minor** bump: Additive changes only (new optional fields, new functions).
  Old plugins continue to work.
- **Patch** bump: Bug fixes to the API crate itself.

The host checks: `plugin.api_version.major == host.api_version.major &&
plugin.api_version.minor <= host.api_version.minor`.

### M2: Version negotiation on load

`prb_plugin_info()` is the **first** function called. It returns the API version
the plugin was compiled against. The host validates compatibility **before**
calling any other function.

### M3: No shared allocator dependency

The plugin allocates memory for `OwnedBuffer` using its own allocator. The host
frees it by calling `prb_plugin_buffer_free()` which routes back to the plugin's
allocator. This avoids cross-allocator issues.

### M4: Freeze the C ABI types

`PluginInfo`, `DetectResultFfi`, `ByteBuffer`, `OwnedBuffer` are intentionally
minimal. Complex data passes through JSON serialization (via `ByteBuffer` containing
JSON bytes), not through C struct fields.

This means the ABI surface is:
- 5 function signatures (info, detect, create, decode, free, destroy)
- 4 small structs (PluginInfo, DetectResultFfi, ByteBuffer, OwnedBuffer)

All complex data (DecodeContext, DebugEventDto) is JSON-serialized inside
`ByteBuffer`, so changes to those types don't affect the ABI.

### M5: Comprehensive CI testing

- Build example plugins against the current API
- Load them in the test suite
- Any API change that breaks the example plugins is caught in CI

## Acceptance Criteria

- API version validation rejects incompatible plugins with clear error messages
- `OwnedBuffer` is allocated and freed by the same plugin (no cross-allocator)
- All complex data crosses the boundary as JSON, not C structs
- Example plugins compile and load successfully
- `prb-plugin-api` has a clear changelog for ABI-affecting changes

---
segment: 3
title: "Patch Security Vulnerabilities"
depends_on: [2]
risk: 6/10
complexity: Medium
cycle_budget: 20
status: pending
commit_message: "fix(security): Patch wasmtime CVEs and upgrade async-openai"
---

# Segment 3: Patch Security Vulnerabilities

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Apply Cargo patches for wasmtime CVEs and upgrade async-openai to resolve security audit failures.

**Depends on:** Segment 2 (needs working builds to test patches)

## Context: Issues Addressed

**Core Problem:** `cargo audit` reports 4 security advisories: (1) wasmtime 37.0.3 has 3 CVEs with severities 6.9, 6.9, and 4.1 allowing resource exhaustion (RUSTSEC-2026-0020), HTTP field panics (RUSTSEC-2026-0021), and potential sandbox escape via f64.copysign on x86-64 (RUSTSEC-2026-0006). (2) backoff 0.4.0 is unmaintained (RUSTSEC-2025-0012, last commit Dec 2021). Wasmtime vulnerabilities come from extism 1.13.0 which hasn't upgraded yet (issue #889 open on their tracker). Backoff comes from async-openai 0.20. All advisories recommend immediate patching for production systems handling untrusted input.

**Proposed Fix:** Use Cargo's `[patch.crates-io]` to force wasmtime 36.0.6 (fixes all 3 CVEs with minimal breaking changes). Upgrade async-openai 0.20 → 0.33.1 to get 13 minor versions of bugfixes and improvements. Accept backoff warning temporarily (async-openai 0.33.1 still uses it, no known vulnerabilities). Document patch removal condition and accepted risks.

**Pre-Mortem Risks:**
- Wasmtime patch might break extism APIs (test full WASM plugin lifecycle with all 40 integration tests)
- async-openai 0.20 → 0.33 likely has breaking API changes (review changelog, update call sites in prb-ai/src/explain.rs)
- Future extism release might conflict with patch (remove patch section when extism upgrades officially, monitor issue #889)
- WASM plugins might behave differently with newer wasmtime (test decode/detect/info functions thoroughly)
- API client initialization might have changed (test streaming responses, error handling)

## Scope

- `/Users/psauer/probe/Cargo.toml` - Workspace root (add `[patch.crates-io]` section)
- `/Users/psauer/probe/crates/prb-ai/Cargo.toml` - Update async-openai version
- `/Users/psauer/probe/crates/prb-tui/Cargo.toml` - Update async-openai version
- Possibly `/Users/psauer/probe/crates/prb-ai/src/explain.rs` - Update API calls if breaking changes
- `/Users/psauer/probe/SECURITY.md` - Document accepted risks and patch removal condition

## Key Files and Context

**Dependency chains:**

Wasmtime vulnerability path:
```
wasmtime 37.0.3
├── wasi-common 37.0.3
├── wasmtime-environ 37.0.3
├── wasmtime-runtime 37.0.3
└── [10+ other wasmtime-* crates]
    └── extism 1.13.0 (Cargo.toml specifies "1.10", but Cargo.lock has 1.13.0)
        └── prb-plugin-wasm 0.1.0
            ├── prb-cli 0.1.0
            └── prb-tui 0.1.0 → prb-cli
```

Backoff unmaintained path:
```
backoff 0.4.0
└── async-openai 0.20.0
    ├── prb-ai 0.1.0 → prb-tui → prb-cli
    └── prb-tui 0.1.0 → prb-cli
```

**Current versions:**
- Workspace Cargo.toml: `extism = "1.10"` (allows 1.10.x - 1.x)
- Cargo.lock: `extism 1.13.0` (latest is 1.13.0, uses wasmtime 37.x)
- Latest extism: `1.13.0` (still uses wasmtime 37.x, no upgrade available yet)
- Cargo.toml: `async-openai = "0.20"`
- Latest async-openai: `0.33.1` (released March 13, 2026 - today!)

**Vulnerabilities details:**

**RUSTSEC-2026-0020** (CVE-2026-27204) - Severity 6.9 (MEDIUM)
- **Title:** Guest-controlled resource exhaustion in WASI implementations
- **Impact:** Malicious WASM plugins can exhaust host memory/CPU, causing DoS
- **Attack vector:** Crafted WASM code making excessive WASI calls
- **Fixed in:** wasmtime >= 36.0.6, >= 40.0.4, or >= 41.0.4
- **URL:** https://rustsec.org/advisories/RUSTSEC-2026-0020

**RUSTSEC-2026-0021** (CVE-2026-27572) - Severity 6.9 (MEDIUM)
- **Title:** Panic when adding excessive fields to `wasi:http/types.fields`
- **Impact:** Malicious WASM can crash the host process
- **Attack vector:** Adding thousands of HTTP header fields
- **Fixed in:** wasmtime >= 36.0.6, >= 40.0.4, or >= 41.0.4
- **URL:** https://rustsec.org/advisories/RUSTSEC-2026-0021

**RUSTSEC-2026-0006** (CVE-2026-24116) - Severity 4.1 (MEDIUM)
- **Title:** Segfault or out-of-sandbox memory access with `f64.copysign` on x86-64
- **Impact:** Potential sandbox escape, memory corruption
- **Attack vector:** Specific WASM instruction on x86-64 architecture
- **Fixed in:** wasmtime >= 36.0.5, >= 40.0.3, or >= 41.0.1
- **URL:** https://rustsec.org/advisories/RUSTSEC-2026-0006

**RUSTSEC-2025-0012** - Severity: WARNING (Unmaintained)
- **Title:** `backoff` crate is unmaintained
- **Impact:** No security patches or bug fixes
- **Last commit:** December 14, 2021 (over 4 years ago)
- **Risk:** Low - simple exponential backoff logic, no network or security-sensitive operations
- **URL:** https://rustsec.org/advisories/RUSTSEC-2025-0012

**Upstream status:**
- Extism issue #889: "wasmtime CVE-2026-24116" - open since Jan 28, 2026, no PR merged yet
- Extism maintainers are aware but haven't released patched version
- async-openai 0.33.1 still uses backoff 0.4.0 (no migration to alternatives yet)

**Files using async-openai:**
- `/Users/psauer/probe/crates/prb-ai/Cargo.toml` - Dependency declaration
- `/Users/psauer/probe/crates/prb-ai/src/explain.rs` (189 lines) - Main usage:
  - Lines 47-73: OpenAI client initialization with API key and base URL
  - Lines 85-88: Empty choices array error handling
  - Lines 90-94: Null content handling
  - Lines 174-183: Streaming response handling with SSE parsing
  - Creates ChatCompletionRequest with messages, model, temperature, max_tokens
- `/Users/psauer/probe/crates/prb-tui/Cargo.toml` - Dependency declaration for TUI AI features

**Files using extism (WASM plugins):**
- `/Users/psauer/probe/crates/prb-plugin-wasm/Cargo.toml` - Dependency: `extism = "1.10"`
- `/Users/psauer/probe/crates/prb-plugin-wasm/src/loader.rs` (182 lines) - Plugin loading and initialization
- `/Users/psauer/probe/crates/prb-plugin-wasm/src/adapter.rs` (98 lines) - WasmDecoderFactory and adapter implementation
- `/Users/psauer/probe/crates/prb-plugin-wasm/tests/loader_test.rs` - 17 integration tests for plugin lifecycle
- `/Users/psauer/probe/crates/prb-plugin-wasm/tests/adapter_test.rs` - 23 integration tests for protocol handling

## Implementation Approach

### Phase 1: Apply Wasmtime Patch

1. **Add Cargo patch section:**

   Edit `/Users/psauer/probe/Cargo.toml`, add after the `[workspace]` section (before any `[workspace.dependencies]`):
   ```toml
   # ============================================================================
   # SECURITY PATCHES - Transitive Dependency Overrides
   # ============================================================================
   # This section forces patched versions of transitive dependencies to fix CVEs.
   # Review and remove patches when upstream crates upgrade.
   #
   # Current patches:
   # - wasmtime 36.0.6: Fixes RUSTSEC-2026-{0006,0020,0021}
   #   Tracking: https://github.com/extism/extism/issues/889
   #   Remove when: extism upgrades to wasmtime 40.0.4+ or 41.0.4+
   #
   [patch.crates-io]
   wasmtime = "36.0.6"
   wasmtime-environ = "36.0.6"
   wasmtime-runtime = "36.0.6"
   wasmtime-jit = "36.0.6"
   wasmtime-cache = "36.0.6"
   wasmtime-cranelift = "36.0.6"
   wasmtime-fiber = "36.0.6"
   wasmtime-component-macro = "36.0.6"
   wasmtime-component-util = "36.0.6"
   wasi-common = "36.0.6"
   wiggle = "36.0.6"
   ```

2. **Update Cargo.lock:**
   ```bash
   cargo update -p wasmtime
   cargo update -p extism
   ```

3. **Verify patched versions:**
   ```bash
   cargo tree -p wasmtime | head -1
   # Expected output: wasmtime v36.0.6

   cargo tree -p extism | head -1
   # Expected output: extism v1.13.0

   cargo tree -i wasmtime | grep -E "wasmtime|extism"
   # Verify dependency chain
   ```

### Phase 2: Test WASM Plugin Functionality

4. **Run full WASM plugin test suite:**
   ```bash
   cargo test -p prb-plugin-wasm
   # Must pass all 40 integration tests

   cargo test -p prb-plugin-wasm loader_test
   # 17 tests: plugin loading, initialization, function calls

   cargo test -p prb-plugin-wasm adapter_test
   # 23 tests: protocol detection, decoding, metadata
   ```

5. **Test specific WASM operations:**
   ```bash
   # Test plugin info extraction
   cargo test -p prb-plugin-wasm test_plugin_info

   # Test protocol detection
   cargo test -p prb-plugin-wasm test_detect

   # Test decoding functionality
   cargo test -p prb-plugin-wasm test_decode
   ```

### Phase 3: Upgrade async-openai

6. **Check async-openai changelog:**
   ```bash
   # Review breaking changes between 0.20 and 0.33
   # URL: https://github.com/64bit/async-openai/blob/main/CHANGELOG.md
   # Common breaking changes in async-openai versions:
   # - Client::new() method signature changes
   # - Request/response struct field names
   # - Error enum variants
   # - Streaming response handling APIs
   ```

7. **Upgrade async-openai dependency:**

   Edit `/Users/psauer/probe/crates/prb-ai/Cargo.toml`:
   ```toml
   [dependencies]
   # Before: async-openai = "0.20"
   async-openai = "0.33"
   ```

   Edit `/Users/psauer/probe/crates/prb-tui/Cargo.toml`:
   ```toml
   [dependencies]
   # Before: async-openai = "0.20"
   async-openai = "0.33"
   ```

8. **Check for compilation errors:**
   ```bash
   cargo build -p prb-ai 2>&1 | tee build_errors.log
   # Review any errors related to async-openai API changes
   ```

9. **Update async-openai call sites if needed:**

   If compilation errors occur, check `/Users/psauer/probe/crates/prb-ai/src/explain.rs`:
   - Client initialization (lines 47-73): `AsyncOpenAI::new()` or `AsyncOpenAI::from_env()`
   - Request creation: `CreateChatCompletionRequestArgs::default()`
   - Response parsing: `ChatCompletionResponse` structure
   - Error handling: `OpenAIError` enum variants
   - Streaming: `ChatCompletionResponseStream` type

   Common migration patterns:
   ```rust
   // Old (0.20):
   use async_openai::{Client, types::{ChatCompletionRequestMessage, ...}};
   let client = Client::new();

   // New (0.33) - typically similar, check docs if errors:
   use async_openai::{Client as AsyncOpenAI, types::{ChatCompletionRequestMessage, ...}};
   let client = AsyncOpenAI::new();
   ```

### Phase 4: Test AI Functionality

10. **Run AI test suite:**
    ```bash
    cargo test -p prb-ai
    # Must pass all 49 existing tests

    cargo test -p prb-ai explain_http
    # Tests OpenAI API mocking with wiremock
    ```

11. **Test specific AI features:**
    ```bash
    # Test streaming responses
    cargo test -p prb-ai test_stream

    # Test error handling
    cargo test -p prb-ai test_error

    # Test empty response handling
    cargo test -p prb-ai test_empty_choices
    ```

### Phase 5: Security Verification

12. **Run full security audit:**
    ```bash
    cargo audit
    # Expected output:
    # - 0 vulnerabilities (wasmtime CVEs fixed)
    # - 1 warning: backoff 0.4.0 unmaintained (accepted risk)
    ```

13. **Verify no new vulnerabilities introduced:**
    ```bash
    cargo audit --json | jq '.vulnerabilities.count'
    # Should output: 0
    ```

### Phase 6: Documentation

14. **Create or update SECURITY.md:**

    Create `/Users/psauer/probe/SECURITY.md`:
    ```markdown
    # Security Policy

    ## Reporting Vulnerabilities

    Please report security vulnerabilities to [security contact].

    ## Dependency Security

    We use `cargo audit` to track security advisories. Run it with:
    ```bash
    cargo audit
    ```

    ### Security Audit Status

    Last audit: 2026-03-13
    - Vulnerabilities: 0
    - Warnings: 1 (accepted)

    ### Known Accepted Risks

    #### RUSTSEC-2025-0012: backoff 0.4.0 unmaintained
    - **Status:** Accepted
    - **Reason:** Indirect dependency via async-openai. No known vulnerabilities, simple exponential backoff logic.
    - **Risk Level:** Low - no network or security-sensitive operations, well-tested code
    - **Upstream Tracking:** Monitoring async-openai for migration to maintained alternatives (backon, tokio-retry)
    - **Last Reviewed:** 2026-03-13
    - **Review Frequency:** Quarterly or when async-openai releases major update

    ### Temporary Patches (Upstream Pending)

    #### Wasmtime Security Fixes (Temporary Patch)
    - **Current Approach:** Using `[patch.crates-io]` to force wasmtime 36.0.6
    - **Fixes:** RUSTSEC-2026-0006, RUSTSEC-2026-0020, RUSTSEC-2026-0021
    - **Reason:** extism 1.13.0 uses wasmtime 37.0.3 with known CVEs
    - **Upstream Tracking:** https://github.com/extism/extism/issues/889
    - **Removal Condition:** Remove `[patch.crates-io]` section when extism upgrades to wasmtime >= 40.0.4 or >= 41.0.4
    - **Testing:** All 40 WASM plugin integration tests pass with patched version
    - **Last Updated:** 2026-03-13

    ## Security Testing

    Security-critical components:
    - WASM plugin sandboxing (extism/wasmtime) - 40 integration tests
    - TLS decryption (ring/rustls) - Wycheproof test vectors
    - Packet parsing (etherparse) - Fuzzing with cargo-fuzz

    ## Dependency Update Policy

    - Monthly `cargo update` to get patch releases
    - Quarterly `cargo audit` review
    - Immediate action on CRITICAL or HIGH severity vulnerabilities
    - MEDIUM severity: Evaluate within 1 week
    - LOW severity: Evaluate within 1 month
    ```

## Alternatives Ruled Out

- **Upgrade to wasmtime 40 or 41 directly:** Rejected - larger version jump (37→40/41), higher risk of breaking changes, wasmtime 36.0.6 fixes all CVEs with minimal risk
- **Fork extism and upgrade wasmtime ourselves:** Rejected - high maintenance burden, merge conflicts on upstream updates, not sustainable long-term
- **Downgrade extism to older version:** Rejected - checked, no older extism versions have patched wasmtime, regression risk
- **Replace extism with wasmer or wasmtime directly:** Rejected - massive refactor (extism provides high-level plugin API that would need complete reimplementation)
- **Replace backoff ourselves in async-openai:** Rejected - backoff is owned by async-openai, would require upstream PR and fork maintenance
- **Disable security audit in CI:** Rejected - violates secure development practices, hides problems instead of fixing them
- **Add vulnerabilities to cargo-audit ignore list:** Rejected - these are real CVEs requiring action, not false positives

## Pre-Mortem Risks

- **Wasmtime 36.0.6 breaking extism:** Mitigation - wasmtime uses careful SemVer, 36.x line is stable, full test suite validation (40 tests)
- **async-openai 0.33 breaking API changes:** Mitigation - review changelog first, update call sites, comprehensive test suite (49 tests)
- **Future extism release conflicting with patch:** Mitigation - monitor issue #889, remove patch immediately when extism upgrades, documented removal condition
- **WASM plugins using wasmtime 37-specific features:** Mitigation - test decode/detect/info extensively, plugin tests cover all critical paths
- **Streaming response handling changed:** Mitigation - test wiremock integration tests specifically for streaming SSE parsing
- **Client initialization API changed:** Mitigation - check compilation errors, refer to async-openai migration guide

## Build and Test Commands

- Build: `cargo build --workspace`
- Build (AI only): `cargo build -p prb-ai`
- Build (WASM only): `cargo build -p prb-plugin-wasm`
- Test (targeted): `cargo test -p prb-plugin-wasm -p prb-ai`
- Test (WASM integration): `cargo test -p prb-plugin-wasm --test loader_test --test adapter_test`
- Test (AI integration): `cargo test -p prb-ai explain_http`
- Test (regression): `cargo test --workspace`
- Test (full gate): `cargo nextest run --workspace`
- Audit: `cargo audit` (should show 0 vulnerabilities, 1 warning)
- Verify patch: `cargo tree -p wasmtime | head -1` (should show 36.0.6)

## Exit Criteria

1. **Targeted tests:**
   - `cargo audit` shows 0 vulnerabilities (only 1 unmaintained warning for backoff is acceptable)
   - `cargo test -p prb-plugin-wasm` passes all 40 integration tests
   - `cargo test -p prb-plugin-wasm loader_test` passes (17 tests)
   - `cargo test -p prb-plugin-wasm adapter_test` passes (23 tests)
   - `cargo test -p prb-ai` passes all 49 tests
   - `cargo test -p prb-ai explain_http` passes (wiremock integration)
   - `cargo tree -p wasmtime | head -1` shows wasmtime v36.0.6
   - `cargo tree -p async-openai | head -1` shows async-openai v0.33.x

2. **Regression tests:**
   - All workspace tests pass: `cargo test --workspace`
   - WASM plugin loading works: verified by loader_test suite
   - AI explain functionality works: verified by explain_http tests
   - No other packages broken by dependency updates

3. **Full build gate:**
   - `cargo build --workspace --all-targets` succeeds
   - `cargo clippy --workspace --all-targets -- -D warnings` passes

4. **Full test gate:**
   - `cargo nextest run --workspace` passes all tests

5. **Self-review gate:**
   - Patch section in Cargo.toml has clear comments explaining:
     - What CVEs are fixed
     - Link to upstream tracking issue
     - Removal condition (when to delete patch)
   - SECURITY.md created or updated with:
     - Accepted risks documentation
     - Patch removal condition
     - Security testing procedures
     - Dependency update policy
   - async-openai API changes properly handled (if any)
   - No wasmtime-37-specific code remaining
   - All comments/TODOs related to security addressed

6. **Scope verification gate:**
   - Modified files match expected list:
     - `Cargo.toml` (workspace root - add `[patch.crates-io]` section)
     - `Cargo.lock` (version updates for wasmtime and async-openai)
     - `crates/prb-ai/Cargo.toml` (async-openai version bump)
     - `crates/prb-tui/Cargo.toml` (async-openai version bump)
     - Possibly `crates/prb-ai/src/explain.rs` (only if API changes required)
     - `SECURITY.md` (new file or updated)
   - No changes to:
     - WASM plugin source code (unless fixing compatibility issues)
     - Test fixture implementation
     - Other workspace crates unrelated to security patches

**Risk factor:** 6/10

**Estimated complexity:** Medium

**Commit message:** `fix(security): Patch wasmtime CVEs and upgrade async-openai`

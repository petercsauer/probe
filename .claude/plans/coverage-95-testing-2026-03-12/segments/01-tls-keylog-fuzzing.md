---
segment: 1
title: "TLS Keylog Parser + Fuzzing Infrastructure"
depends_on: []
risk: 8/10
complexity: Medium
cycle_budget: 15
status: merged
commit_message: "test(pcap-tls): Add comprehensive keylog parser tests and fuzzing infrastructure"
---

# Segment 1: TLS Keylog Parser + Fuzzing Infrastructure

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Increase keylog.rs coverage from 1.55% to 85%+ and establish fuzzing infrastructure for the workspace.

**Depends on:** None

## Context: Issues Addressed

**Core Problem:** The TLS keylog parser has critical security vulnerabilities with only 1.55% test coverage. Line 135 contains `from_utf8().unwrap()` which will panic on malformed input. File parsing has minimal error validation (lines 80-97). The parser supports both TLS 1.2 (48-byte master secrets) and TLS 1.3 (32/48-byte traffic secrets) but only happy-path scenarios are tested.

**Proposed Fix:** Replace unwrap with proper error handling, create comprehensive malformed input corpus (50+ test cases), add parameterized tests with rstest for all key types, establish cargo-fuzz infrastructure for the workspace, and add property tests for round-trip validation.

**Pre-Mortem Risks:**
- Fuzzing might find issues in upstream `hex` crate - document as external dependency issue
- CI time for fuzzing - mitigate by running fuzzing nightly in separate job
- Corpus size explosion - limit to 100 malformed cases max

## Scope

- `crates/prb-pcap/src/tls/keylog.rs` - Keylog parser (268 lines)
- `crates/prb-pcap/tests/keylog_tests.rs` - Existing tests
- `crates/prb-pcap/tests/corpus/keylog_malformed.rs` - New malformed input tests
- `fuzz/` - New fuzzing infrastructure for workspace
- Workspace `Cargo.toml` - Add fuzz member

## Key Files and Context

**`crates/prb-pcap/src/tls/keylog.rs`** (268 lines):
- Line 135: `from_utf8().unwrap()` - Critical panic risk on malformed input
- Lines 80-97: File parsing with minimal error validation
- Lines 99-165: Line parsing for CLIENT_RANDOM, CLIENT_TRAFFIC_SECRET_0, etc.
- Supports TLS 1.2 (48-byte master secret) and TLS 1.3 (32/48-byte traffic secrets)

**`crates/prb-pcap/tests/keylog_tests.rs`**:
- Existing tests use tempfile::NamedTempFile
- Current coverage: 1.55% indicates only happy path tested

**Fuzzing infrastructure:** None exists yet in workspace

## Implementation Approach

1. **Replace unwrap at line 135** with Result-based error handling:
   ```rust
   let label = std::str::from_utf8(label_bytes)
       .map_err(|e| PcapError::InvalidKeylog(format!("Invalid UTF-8 in label: {}", e)))?;
   ```

2. **Create malformed input corpus** in `tests/corpus/keylog_malformed.rs` (50+ cases):
   - Invalid hex encoding: odd length, non-hex chars ("GGGG"), mixed case
   - Wrong key lengths: 31 bytes, 49 bytes, 0 bytes, 1000 bytes
   - Missing client_random in line
   - Invalid label names: "INVALID_LABEL", empty string
   - Non-UTF8 bytes in comments or labels
   - Empty lines, lines with only whitespace, lines with only comments
   - Duplicate client_random entries
   - Mixed TLS 1.2 and 1.3 keys in same file

3. **Add parameterized tests** with rstest:
   ```rust
   #[rstest]
   #[case::tls12_master_secret("CLIENT_RANDOM", 48)]
   #[case::tls13_client_traffic("CLIENT_TRAFFIC_SECRET_0", 32)]
   #[case::tls13_server_traffic("SERVER_TRAFFIC_SECRET_0", 32)]
   #[case::tls13_client_handshake("CLIENT_HANDSHAKE_TRAFFIC_SECRET", 32)]
   #[case::tls13_server_handshake("SERVER_HANDSHAKE_TRAFFIC_SECRET", 32)]
   fn test_key_type_parsing(#[case] label: &str, #[case] expected_len: usize) { ... }
   ```

4. **Set up cargo-fuzz**:
   - Add to workspace Cargo.toml: `[workspace] members = ["fuzz"]`
   - Create `fuzz/Cargo.toml` with `cargo-fuzz` template
   - Create `fuzz/fuzz_targets/keylog_parser.rs` targeting `TlsKeyLog::parse_line()`
   - Add seed corpus from existing test cases

5. **Add property tests** with proptest for round-trip generation:
   ```rust
   proptest! {
       #[test]
       fn keylog_roundtrip(client_random in "[0-9a-f]{64}", master_secret in "[0-9a-f]{96}") {
           let line = format!("CLIENT_RANDOM {} {}", client_random, master_secret);
           let parsed = TlsKeyLog::parse_line(&line);
           assert!(parsed.is_ok());
       }
   }
   ```

## Alternatives Ruled Out

- **Fuzzing only without malformed corpus:** Rejected - need deterministic test cases for CI
- **Manual test enumeration without rstest:** Rejected - too verbose for key type × format matrix (5 types × 10 edge cases)

## Pre-Mortem Risks

- Fuzzing might find issues in `hex` crate (upstream): Document as external dependency issue, file upstream bug
- CI time for fuzzing: Mitigate by running fuzzing nightly in separate job, not per-commit
- Corpus size explosion: Limit to 100 malformed cases max to keep test suite fast

## Build and Test Commands

- Build: `cargo build -p prb-pcap --all-features`
- Test (targeted): `cargo test -p prb-pcap keylog_malformed keylog_property`
- Test (regression): `cargo test -p prb-pcap tls`
- Test (full gate): `cargo nextest run -p prb-pcap`
- Fuzz (optional): `cargo fuzz run keylog_parser -- -max_total_time=60`

## Exit Criteria

1. **Targeted tests:**
   - `keylog_malformed_corpus` - 50+ malformed input cases pass (verify they return errors, not panics)
   - `keylog_property_roundtrip` - proptest passes with 100+ generated test cases
   - `keylog_rstest_matrix` - 5 key types × multiple edge cases all pass

2. **Regression tests:** All existing TLS tests in `tests/tls_tests.rs`, `tests/keylog_tests.rs` pass

3. **Full build gate:** `cargo build -p prb-pcap --all-features` succeeds with zero warnings

4. **Full test gate:** `cargo nextest run -p prb-pcap` passes

5. **Self-review gate:**
   - No dead code, no TODO/HACK comments
   - Unwrap at line 135 replaced with proper error handling
   - Fuzzing infrastructure documented in workspace README or docs/

6. **Scope verification gate:** Only modified:
   - `keylog.rs` - error handling changes
   - Test files in `tests/` directory
   - Added `fuzz/` directory
   - Updated workspace `Cargo.toml` to include fuzz member

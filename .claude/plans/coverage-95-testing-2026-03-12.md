# Coverage 95% Testing Plan

**Created:** 2026-03-12
**Goal:** Increase Rust codebase test coverage from ~82% to 95%
**Status:** Ready for execution
**Segments:** 9
**Estimated effort:** 122 cycles (~12-15 hours)

## Execution Order (Fail-Fast)

1. Segment 1 (prerequisite) → Segment 2 (Risk 9/10)
2. Segment 5 (prerequisite) → Segment 6 (Risk 8/10)
3. Segment 4 (Risk 6/10) - Can run parallel with others
4. Segment 3 (Risk 6/10) - After Segment 2
5. Segment 7 (Risk 5/10) - Independent
6. Segment 8 (Risk 4/10) - Independent
7. Segment 9 (Risk 4/10) - Independent

## Parallelization
- Wave 3: Segments 3, 4, 6, 7 can run concurrently (max 4 parallel)
- Wave 4: Segments 8, 9 can run concurrently

---

## Segment 1: TLS Keylog Parser + Fuzzing Infrastructure
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Increase keylog.rs coverage from 1.55% to 85%+ and establish fuzzing infrastructure for the workspace.

**Depends on:** None

**Issues addressed:** TLS Keylog Parser (critical security component)

**Cycle budget:** 15 Medium

**Scope:** `crates/prb-pcap/src/tls/keylog.rs`, fuzzing setup

**Key files and context:**
- `/Users/psauer/probe/crates/prb-pcap/src/tls/keylog.rs` - Keylog parser (268 lines)
  - Line 135: `from_utf8().unwrap()` - Critical panic risk on malformed input
  - Lines 80-97: File parsing with minimal error validation
  - Lines 99-165: Line parsing for CLIENT_RANDOM, CLIENT_TRAFFIC_SECRET_0, etc.
  - Supports TLS 1.2 (48-byte master secret) and TLS 1.3 (32/48-byte traffic secrets)
- `/Users/psauer/probe/crates/prb-pcap/tests/keylog_tests.rs` - Existing tests use tempfile::NamedTempFile
- Fuzzing infrastructure: None exists yet in workspace
- Current coverage: 1.55% indicates only happy path tested

**Implementation approach:**
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

**Alternatives ruled out:**
- Fuzzing only without malformed corpus: Rejected—need deterministic test cases for CI
- Manual test enumeration without rstest: Rejected—too verbose for key type × format matrix (5 types × 10 edge cases)

**Pre-mortem risks:**
- Fuzzing might find issues in `hex` crate (upstream): Document as external dependency issue, file upstream bug
- CI time for fuzzing: Mitigate by running fuzzing nightly in separate job, not per-commit
- Corpus size explosion: Limit to 100 malformed cases max to keep test suite fast

**Segment-specific commands:**
- Build: `cargo build -p prb-pcap --all-features`
- Test (targeted): `cargo test -p prb-pcap keylog_malformed keylog_property`
- Test (regression): `cargo test -p prb-pcap tls`
- Test (full gate): `cargo nextest run -p prb-pcap`
- Fuzz (optional): `cargo fuzz run keylog_parser -- -max_total_time=60`

**Exit criteria:**
1. Targeted tests: `keylog_malformed_corpus` (50+ cases pass), `keylog_property_roundtrip` (proptest passes), `keylog_rstest_matrix` (5 key types pass)
2. Regression tests: All existing TLS tests in `tests/tls_tests.rs`, `tests/keylog_tests.rs` pass
3. Full build gate: `cargo build -p prb-pcap --all-features` succeeds with zero warnings
4. Full test gate: `cargo nextest run -p prb-pcap` passes
5. Self-review gate: No dead code, no TODO/HACK comments, unwrap at line 135 replaced with proper error handling, fuzzing infrastructure documented
6. Scope verification gate: Only modified `keylog.rs`, test files, added `fuzz/` directory, updated workspace `Cargo.toml`

**Risk factor:** 8/10

**Estimated complexity:** Medium

**Commit message:** `test(pcap-tls): Add comprehensive keylog parser tests and fuzzing infrastructure`

---

## Segment 2: TLS Decryption RFC and Wycheproof Vectors
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Increase decrypt.rs coverage from 25.58% to 75%+ using authoritative RFC and Wycheproof test vectors.

**Depends on:** Segment 1 (fuzzing infrastructure can be reused)

**Issues addressed:** TLS Decryption Core (cryptographic correctness, highest risk)

**Cycle budget:** 20 High

**Scope:** `crates/prb-pcap/src/tls/decrypt.rs`, `kdf.rs`, test vectors

**Key files and context:**
- `/Users/psauer/probe/crates/prb-pcap/src/tls/decrypt.rs` (404 lines)
  - Lines 96-97: Silent zero-key creation `(vec![0u8; len], vec![0u8; 12])` instead of error - security issue
  - Lines 238-325: AEAD decryption with nonce/AAD construction for TLS 1.2 and 1.3
  - Lines 284-286: Nonce XOR assumes 8-byte boundary for TLS 1.2 explicit nonce
  - Uses `ring::aead` for AES-GCM and ChaCha20-Poly1305
- `/Users/psauer/probe/crates/prb-pcap/src/tls/kdf.rs` (261 lines)
  - TLS 1.2 PRF using HMAC-based pseudo-random function
  - TLS 1.3 HKDF-Expand-Label for key derivation
  - SHA256 and SHA384 variants
- `/Users/psauer/probe/crates/prb-pcap/tests/tls_tests.rs` - Existing synthetic encrypt/decrypt helpers using `ring::aead::SealingKey`
- Current: Only AES-128-GCM and AES-256-GCM tested, missing ChaCha20-Poly1305 and SHA384 variants
- Multi-record streams with sequence number progression untested

**Implementation approach:**
1. **Fix silent failure** at lines 96-97:
   ```rust
   return Err(TlsError::MissingKeyMaterial(format!(
       "No {} key material found for client_random",
       if direction == Direction::ClientToServer { "client" } else { "server" }
   )));
   ```
2. **Add RFC test vectors** in `tests/rfc_vectors/`:
   - RFC 5869 Appendix A: HKDF test vectors (7 cases covering SHA256/SHA384, various IKM/salt/info lengths)
     - Create `tests/rfc_vectors/rfc5869_hkdf.json` with test cases
   - RFC 8446 Appendix B: TLS 1.3 key derivation (3 cases for traffic secrets)
     - Create `tests/rfc_vectors/rfc8446_tls13.json`
   - RFC 5246 Appendix A.5: TLS 1.2 PRF (2 cases)
     - Create `tests/rfc_vectors/rfc5246_tls12.json`
   - Create parser in `tests/rfc_vectors/mod.rs` to load and execute JSON test vectors
3. **Add Wycheproof AEAD vectors**:
   - Download from github.com/google/wycheproof:
     - `aes_gcm_test.json` (~300 test cases)
     - `chacha20_poly1305_test.json` (~200 test cases)
   - Place in `tests/fixtures/wycheproof/`
   - Create `tests/wycheproof_runner.rs` to execute vectors
   - Test cases cover: valid encryption, invalid auth tags, edge case nonce/AAD lengths, zero-length messages
4. **Add multi-record tests** in `tests/tls_multi_record_test.rs`:
   - Test sequence number progression (0, 1, 2, ..., 100)
   - Test sequence wrap at 2^64-1 (implementation should handle or error)
   - Test record boundaries: multiple records in single stream, partial records
   - Test out-of-order records (should fail authentication)
5. **Property tests** with proptest in `tests/tls_property_test.rs`:
   ```rust
   proptest! {
       #[test]
       fn encrypt_decrypt_roundtrip(
           key in prop::collection::vec(any::<u8>(), 16..=32),
           nonce in prop::collection::vec(any::<u8>(), 12),
           plaintext in prop::collection::vec(any::<u8>(), 0..1000)
       ) {
           let ciphertext = encrypt(&key, &nonce, &plaintext);
           let decrypted = decrypt(&key, &nonce, &ciphertext);
           assert_eq!(decrypted, plaintext);
       }
   }
   ```

**Alternatives ruled out:**
- Only RFC vectors without Wycheproof: Rejected—misses adversarial edge cases like tampered auth tags
- Only synthetic tests without official vectors: Rejected—doesn't validate standards compliance with authoritative sources

**Pre-mortem risks:**
- Wycheproof vectors might expose bugs in `ring` library: Document as external issue, report to rust-crypto/ring project
- 500+ test cases could slow CI: Mitigate with nextest parallelization (already in workspace)
- RFC vectors might not match implementation due to spec ambiguity: Cross-reference with rustls behavior as canonical implementation

**Segment-specific commands:**
- Build: `cargo build -p prb-pcap --features tls`
- Test (targeted): `cargo test -p prb-pcap rfc_vectors wycheproof multi_record tls_property`
- Test (regression): `cargo test -p prb-pcap tls`
- Test (full gate): `cargo nextest run -p prb-pcap`

**Exit criteria:**
1. Targeted tests: `rfc_vectors` (12 RFC cases pass), `wycheproof_aead` (500+ cases pass), `multi_record_sequence` (sequence progression passes), `tls_property` (proptest passes)
2. Regression tests: All existing TLS tests pass including `tests/tls_tests.rs`, `tests/tls_decrypt_edge_tests.rs`
3. Full build gate: `cargo build -p prb-pcap --features tls` succeeds with zero warnings
4. Full test gate: `cargo nextest run -p prb-pcap` passes
5. Self-review gate: Silent failure at lines 96-97 replaced with error, no test-only code in production paths, Wycheproof JSON documented
6. Scope verification gate: Only modified `decrypt.rs`, `kdf.rs`, test files; added `tests/rfc_vectors/`, `tests/fixtures/wycheproof/`

**Risk factor:** 9/10

**Estimated complexity:** High

**Commit message:** `test(pcap-tls): Add RFC and Wycheproof test vectors for AEAD decryption`

---

## Segment 3: TLS Cipher Suite Coverage
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Test all 9 supported cipher suites (currently only 3-4 tested).

**Depends on:** Segment 2 (AEAD testing framework established)

**Issues addressed:** TLS Decryption Core - cipher suite gap

**Cycle budget:** 10 Low

**Scope:** Test coverage for all cipher suites (ChaCha20-Poly1305, SHA384 variants)

**Key files and context:**
- `/Users/psauer/probe/crates/prb-pcap/src/tls/session.rs` - Cipher suite mapping
  - Lines 80-120: 9 cipher suites defined with hex IDs
- Supported cipher suites:
  - AES-128-GCM: 0x009C, 0x009E, 0xC02F, 0xC02B, 0x1301 (TLS 1.3)
  - AES-256-GCM: 0x009D, 0x009F, 0xC030, 0xC02C, 0x1302 (TLS 1.3)
  - ChaCha20-Poly1305: 0x1303 (TLS 1.3), 0xCCA8, 0xCCA9 (TLS 1.2)
- Currently tested: AES-128-GCM (0x1301), AES-256-GCM (0x1302) only ~3 of 9
- Untested: ChaCha20-Poly1305 variants, SHA384-based cipher suites

**Implementation approach:**
1. **Create test matrix** with rstest in `tests/cipher_suite_coverage_test.rs`:
   ```rust
   #[rstest]
   #[case::aes128_gcm_0x009c(0x009C, "AES-128-GCM", 16, 12)]
   #[case::aes128_gcm_0x009e(0x009E, "AES-128-GCM", 16, 12)]
   #[case::aes128_gcm_0xc02f(0xC02F, "AES-128-GCM", 16, 12)]
   #[case::aes128_gcm_0xc02b(0xC02B, "AES-128-GCM", 16, 12)]
   #[case::aes128_gcm_0x1301(0x1301, "AES-128-GCM-TLS13", 16, 12)]
   #[case::aes256_gcm_0x009d(0x009D, "AES-256-GCM", 32, 12)]
   #[case::aes256_gcm_0x009f(0x009F, "AES-256-GCM", 32, 12)]
   #[case::aes256_gcm_0xc030(0xC030, "AES-256-GCM", 32, 12)]
   #[case::aes256_gcm_0xc02c(0xC02C, "AES-256-GCM", 32, 12)]
   #[case::aes256_gcm_0x1302(0x1302, "AES-256-GCM-TLS13", 32, 12)]
   #[case::chacha20_0x1303(0x1303, "ChaCha20-Poly1305-TLS13", 32, 12)]
   #[case::chacha20_0xcca8(0xCCA8, "ChaCha20-Poly1305", 32, 12)]
   #[case::chacha20_0xcca9(0xCCA9, "ChaCha20-Poly1305", 32, 12)]
   fn test_cipher_suite(
       #[case] cipher_id: u16,
       #[case] name: &str,
       #[case] key_len: usize,
       #[case] nonce_len: usize
   ) {
       // Generate synthetic key material
       // Create minimal TLS session with this cipher suite
       // Encrypt plaintext with ring::aead
       // Decrypt with TlsDecryptor
       // Verify plaintext matches
   }
   ```
2. **Generate synthetic test data** for each cipher suite:
   - Use `ring::aead::LessSafeKey` with each algorithm
   - Create test plaintext "Hello, TLS cipher suite test!"
   - Encrypt to produce valid ciphertext + auth tag
3. **Test end-to-end** decryption for each:
   - Parse cipher suite ID from session
   - Derive keys using KDF (TLS 1.2 or 1.3 as appropriate)
   - Decrypt and verify plaintext matches
4. **Add real PCAP fixtures** (optional enhancement):
   - If available, add ChaCha20 PCAP from mobile browser capture
   - If available, add SHA384 PCAP from high-security context
   - Place in `tests/fixtures/captures/tls/` with corresponding keylog files

**Alternatives ruled out:**
- Testing only common cipher suites (AES-128/256): Rejected—production uses all 9 (ChaCha20 for mobile, SHA384 for high-security), must validate all
- Property testing across cipher suites: Rejected—adds complexity without value, explicit per-suite tests more readable

**Pre-mortem risks:**
- ChaCha20-Poly1305 might have `ring` API differences from GCM: Pin ring version 0.17 and document API usage
- Real PCAP fixtures might be hard to find: Synthetic tests sufficient for coverage, real fixtures optional enhancement

**Segment-specific commands:**
- Build: `cargo build -p prb-pcap --features tls`
- Test (targeted): `cargo test -p prb-pcap cipher_suite_coverage`
- Test (regression): `cargo test -p prb-pcap tls`
- Test (full gate): `cargo nextest run -p prb-pcap`

**Exit criteria:**
1. Targeted tests: `cipher_suite_coverage` (13 cipher suite tests pass: 9 unique + 4 ID variants)
2. Regression tests: All TLS tests pass
3. Full build gate: `cargo build -p prb-pcap --features tls` succeeds
4. Full test gate: `cargo nextest run -p prb-pcap` passes
5. Self-review gate: No skipped tests, all 9 cipher suites validated, ChaCha20 thoroughly tested
6. Scope verification gate: Only test files modified, no production code changes (cipher suite mapping already exists)

**Risk factor:** 6/10

**Estimated complexity:** Low

**Commit message:** `test(pcap-tls): Add coverage for all 9 cipher suites including ChaCha20`

---

## Segment 4: Protobuf Testing Suite
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Increase protobuf coverage from 32-69% to 85%+ with systematic type testing and edge cases.

**Depends on:** None (independent)

**Issues addressed:** Protobuf Decoding Edge Cases

**Cycle budget:** 18 High

**Scope:** `crates/prb-decode/src/schema_backed.rs`, `wire_format.rs`

**Key files and context:**
- `/Users/psauer/probe/crates/prb-decode/src/schema_backed.rs` (356 lines)
  - Lines 192-195: Unbounded recursion in `format_value()` for nested messages - stack overflow risk
  - Uses `prost-reflect` for dynamic protobuf decoding without generated code
  - Current coverage: 32.55% (69.55% line coverage)
- `/Users/psauer/probe/crates/prb-decode/src/wire_format.rs` (513 lines)
  - Lines 138-180: Recursive descent parser with MAX_RECURSION_DEPTH = 64
  - Current coverage: 69.74% (missing edge cases)
- Existing tests use manual descriptor building with `prost_types::FileDescriptorProto`
- Current gaps: No property testing, no fuzzing, limited malformed input testing

**Implementation approach:**
1. **Add recursion depth limit** to schema_backed formatter at lines 192-195:
   ```rust
   fn format_value(f: &mut fmt::Formatter, value: &Value, indent: usize, depth: usize) -> fmt::Result {
       const MAX_DEPTH: usize = 64;
       if depth >= MAX_DEPTH {
           return write!(f, "<max recursion depth reached>");
       }
       match value {
           Value::Message(msg) => {
               writeln!(f, "{{")?;
               format_message_fields(f, msg, indent + 1, depth + 1)?; // Pass depth
               write!(f, "{}}}", "  ".repeat(indent))
           }
           // ... other cases
       }
   }
   ```
2. **Create descriptor builder utility** in `tests/helpers/descriptor_builder.rs`:
   ```rust
   pub struct DescriptorBuilder {
       name: String,
       package: String,
       fields: Vec<(String, i32, FieldType)>,
   }

   impl DescriptorBuilder {
       pub fn message(name: &str) -> Self { /* ... */ }
       pub fn field(mut self, name: &str, num: i32, field_type: FieldType) -> Self { /* ... */ }
       pub fn build(self) -> MessageDescriptor { /* ... */ }
   }

   // Usage:
   let desc = DescriptorBuilder::message("TestMsg")
       .field("id", 1, FieldType::Int32)
       .field("name", 2, FieldType::String)
       .build();
   ```
3. **Add parameterized tests** with rstest for all 18 protobuf types in `tests/protobuf_type_matrix_test.rs`:
   - Scalar: int32, int64, uint32, uint64, sint32, sint64, bool, string, bytes (9 types)
   - Fixed: fixed32, fixed64, sfixed32, sfixed64, float, double (6 types)
   - Complex: message, enum, repeated (3 types)
   - Test matrix: Each type × [zero, min, max, normal, negative] values = 90 test cases
   ```rust
   #[rstest]
   #[case::int32_zero(FieldType::Int32, 0i32, vec![0x08, 0x00])]
   #[case::int32_max(FieldType::Int32, i32::MAX, vec![0x08, 0xff, 0xff, 0xff, 0xff, 0x07])]
   #[case::uint64_max(FieldType::Uint64, u64::MAX, vec![0x08, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01])]
   // ... 87 more cases
   fn test_protobuf_type(
       #[case] field_type: FieldType,
       #[case] value: impl Into<Value>,
       #[case] expected_bytes: Vec<u8>
   ) { /* ... */ }
   ```
4. **Create malformed input corpus** in `tests/corpus/protobuf_malformed.rs` (50+ cases):
   - Invalid varint: no terminator byte (10 bytes all with high bit set), >10 bytes
   - Truncated messages: truncated at tag byte, in middle of varint, in length-delimited field, in fixed32/64
   - Invalid UTF-8 in string fields: `vec![0x12, 0x04, 0xff, 0xfe, 0xfd, 0xfc]`
   - Field number 0 (invalid per spec): `vec![0x00, 0x01]`
   - Recursion bombs: deeply nested messages at depths 100, 1000
   - Reserved wire types 3, 4, 6, 7: `vec![0x1b, 0x01]` (field 3, wire type 3)
   - Zero-length strings/bytes, length overflow (length > remaining bytes)
5. **Add proptest strategies** for round-trip testing in `tests/protobuf_property_test.rs`:
   ```rust
   fn arb_protobuf_message() -> impl Strategy<Value = (MessageDescriptor, Vec<u8>)> {
       // Generate arbitrary valid protobuf messages
       // Return descriptor + encoded bytes
   }

   proptest! {
       #[test]
       fn roundtrip_encode_decode((desc, encoded) in arb_protobuf_message()) {
           let decoded = decode_with_schema(&encoded, &desc).unwrap();
           let re_encoded = encode_message(&decoded);
           assert_eq!(re_encoded, encoded);
       }
   }
   ```
6. **Add fuzzing target** in `fuzz/fuzz_targets/protobuf_decoder.rs`:
   ```rust
   #![no_main]
   use libfuzzer_sys::fuzz_target;
   use prb_decode::decode_wire_format;

   fuzz_target!(|data: &[u8]| {
       let _ = decode_wire_format(data); // Should never panic
   });
   ```

**Alternatives ruled out:**
- Manual enumeration of 90 type combinations: Rejected—rstest cleaner, auto-generates test names, easier to review
- Testing only common types (int32, string, message): Rejected—production uses all types including fixed32/64, sfixed, etc.

**Pre-mortem risks:**
- Recursion limit on formatter could break legitimate deeply nested messages: Make configurable via env var, default 64 like wire parser
- Proptest round-trips might fail on unknown fields: Preserve unknown fields during encoding
- rstest adds compilation time for 90 test instances: Acceptable tradeoff, tests still fast at runtime

**Segment-specific commands:**
- Build: `cargo build -p prb-decode`
- Test (targeted): `cargo test -p prb-decode protobuf_type_matrix malformed_corpus protobuf_property`
- Test (regression): `cargo test -p prb-decode`
- Test (full gate): `cargo nextest run -p prb-decode`
- Fuzz (optional): `cargo fuzz run protobuf_decoder -- -max_total_time=60`

**Exit criteria:**
1. Targeted tests: `protobuf_type_matrix` (90 tests pass), `malformed_corpus` (50+ cases pass), `protobuf_property` (proptest passes)
2. Regression tests: All existing decode tests in `tests/schema_backed_tests.rs`, `tests/wire_format_tests.rs` pass
3. Full build gate: `cargo build -p prb-decode` succeeds with zero warnings
4. Full test gate: `cargo nextest run -p prb-decode` passes
5. Self-review gate: Recursion limit added with configurable depth, descriptor builder eliminates test boilerplate, fuzzing target works
6. Scope verification gate: Only modified `schema_backed.rs`, `wire_format.rs`, test files; added rstest dependency to Cargo.toml

**Risk factor:** 6/10

**Estimated complexity:** High

**Commit message:** `test(decode): Add comprehensive protobuf type coverage with fuzzing and property tests`

---

## Segment 5: Packet Normalization Memory Safety
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Increase normalize.rs coverage from 45.80% to 80%+ and validate memory safety with defragmentation.

**Depends on:** None (independent)

**Issues addressed:** Packet Normalization (memory leak, timeout logic, link-layer edge cases)

**Cycle budget:** 15 Medium

**Scope:** `crates/prb-pcap/src/normalize.rs`

**Key files and context:**
- `/Users/psauer/probe/crates/prb-pcap/src/normalize.rs` (791 lines)
  - Line 354: `Box::leak(payload_owned.into_boxed_slice())` - Intentional memory leak for every reassembled IP fragment, no cleanup mechanism
  - Line 170: `saturating_sub(DEFRAG_TIMEOUT_US)` for timeout could prevent cleanup on timestamp wraparound
  - Lines 373-396: Manual TCP header parsing with array indexing after only checking `data.len() >= 20`
  - Supports link layers: Ethernet (1), Raw IP (101), SLL (113), SLL2 (276), Loopback (0)
- `/Users/psauer/probe/crates/prb-pcap/tests/normalize_tests.rs` - Existing tests use etherparse packet construction
- Current coverage: 45.80% indicates defragmentation lifecycle, timeout handling, and link-layer variants undertested

**Implementation approach:**
1. **Add IP defragmentation lifecycle tests** in `tests/normalize_lifecycle_test.rs`:
   - Test fragment reassembly: 3 fragments → reassembled packet
   - Test cleanup after timeout: Send fragment, advance time 5+ seconds, verify cleanup
   - Test 10K fragmented packets with memory profiling (track heap usage pattern)
   - Test timestamp wraparound: Start at u64::MAX - 1000, send fragments, verify cleanup works
   - Test backwards time: Send packets with decreasing timestamps, verify no panic
   - Test huge time gaps: 1 year gap between packets, verify cleanup
2. **Add property tests** for TCP header parsing in `tests/normalize_property_test.rs`:
   ```rust
   proptest! {
       #[test]
       fn tcp_header_parsing_never_panics(
           header_bytes in prop::collection::vec(any::<u8>(), 20..60)
       ) {
           // Should never panic even with arbitrary bytes
           let result = parse_tcp_header(&header_bytes);
           // Can succeed or fail, but no panic
       }
   }
   ```
3. **Add link-layer edge case tests** in `tests/normalize_link_edge_test.rs`:
   - SLL2 with truncated headers (header claims length 20 but only 10 bytes available)
   - Loopback with invalid AF families (not AF_INET/AF_INET6)
   - VLAN with max depth (4 nested VLAN tags, 0x8100 repeated)
   - Truncated Ethernet frames (14 byte header but claims to have payload)
   - Zero-length Ethernet frames
4. **Add memory profiling test** in `tests/normalize_memory_test.rs` (optional benchmark):
   ```rust
   #[test]
   #[ignore] // Run with --ignored flag
   fn test_fragment_memory_usage() {
       let mut normalizer = PacketNormalizer::new();
       let start_memory = get_heap_usage(); // Use allocator stats

       // Process 10K fragmented packets
       for i in 0..10_000 {
           let fragment = create_ip_fragment(i);
           normalizer.normalize(1, i * 1000, &fragment);
       }

       let mid_memory = get_heap_usage();

       // Trigger cleanup by advancing time
       normalizer.cleanup_old_fragments(10_000_000_000); // 10K seconds later

       let end_memory = get_heap_usage();

       // Document expected behavior: memory grows then stabilizes
       println!("Memory: start={}, mid={}, end={}", start_memory, mid_memory, end_memory);
       // Box::leak means leaked memory never freed, document this
   }
   ```
5. **Consider Box::leak refactor** (optional, document if deferred):
   - If tests show unbounded growth is problematic, add TODO with issue tracker reference
   - Alternative: Arena allocator with fragment pool lifetime management
   - Decision: Document current behavior in code comments with rationale
   ```rust
   // SAFETY: We intentionally leak reassembled fragment payloads here to satisfy the
   // 'static lifetime requirement for NormalizedPacket. In practice, fragments are
   // rare and this leak is bounded by the defragmentation timeout (5 seconds).
   // For long-running captures with heavy fragmentation, consider using an arena
   // allocator with explicit lifetime management. See issue #XXX for tracking.
   ```

**Alternatives ruled out:**
- Ignoring Box::leak issue: Rejected—tests will document memory growth pattern, making issue visible for future architecture decisions
- Rewriting defrag logic without Box::leak: Rejected—too invasive for coverage-focused task, defer to separate architecture refactor

**Pre-mortem risks:**
- Memory tests could be flaky on allocator behavior: Use jemalloc or system allocator with consistent config
- Property tests might be slow with arbitrary packet generation: Limit to 1000 iterations with `proptest! { #![proptest_config(ProptestConfig::with_cases(1000))] }`
- Timestamp edge cases might be environment-dependent: Use monotonic timestamps in tests, not wall clock

**Segment-specific commands:**
- Build: `cargo build -p prb-pcap`
- Test (targeted): `cargo test -p prb-pcap normalize_lifecycle normalize_property normalize_link_edge`
- Test (regression): `cargo test -p prb-pcap normalize`
- Test (full gate): `cargo nextest run -p prb-pcap`
- Memory check (optional): `cargo test -p prb-pcap normalize_memory -- --ignored`

**Exit criteria:**
1. Targeted tests: `normalize_lifecycle` (6 tests for reassembly/timeout/wraparound pass), `normalize_property_tcp` (proptest no panics), `normalize_link_edge` (5 link-layer edge cases pass)
2. Regression tests: All normalize tests in `tests/normalize_tests.rs`, `tests/normalize_edge_tests.rs` pass
3. Full build gate: `cargo build -p prb-pcap` succeeds with zero warnings
4. Full test gate: `cargo nextest run -p prb-pcap` passes
5. Self-review gate: Box::leak documented with SAFETY comment explaining rationale and tradeoffs, or TODO added if refactor planned
6. Scope verification gate: Only modified `normalize.rs` and test files, no production behavior changes

**Risk factor:** 7/10

**Estimated complexity:** Medium

**Commit message:** `test(pcap-normalize): Add defrag lifecycle and memory safety tests`

---

## Segment 6: Pipeline Core Robustness
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Increase pipeline_core.rs coverage from 21.38% to 75%+ and fix hot path safety issues.

**Depends on:** Segment 5 (normalization fixes might surface pipeline issues)

**Issues addressed:** Pipeline Core Hot Path (panic risks, silent errors, unbounded warnings)

**Cycle budget:** 12 Medium

**Scope:** `crates/prb-pcap/src/pipeline_core.rs`

**Key files and context:**
- `/Users/psauer/probe/crates/prb-pcap/src/pipeline_core.rs` (280 lines)
  - Line 276: `.unwrap()` after `.is_empty()` check - race condition or logic error could panic in hot path
  - Lines 222-228: Silent error swallowing - all decoder errors converted to fallback events, masks failures
  - Lines 120, 162, 179: Unbounded warning accumulation - `warnings.push()` without capacity limit, memory exhaustion risk
  - Hot path: Called once per packet, must never panic
- `/Users/psauer/probe/crates/prb-pcap/tests/pipeline_tests.rs` - Existing tests focus on happy path
- Current coverage: 21.38% indicates error injection and edge case handling completely untested

**Implementation approach:**
1. **Replace unwrap at line 276** with defensive error handling:
   ```rust
   // Before:
   Some(events.into_iter().next().unwrap())

   // After:
   match events.into_iter().next() {
       Some(event) => Some(event),
       None => {
           error!("Unexpected empty events after non-empty check at {}:{}", file!(), line!());
           stats.unexpected_empty_events += 1;
           None
       }
   }
   ```
2. **Add warning capacity limit** with LRU eviction:
   - Add `lru = "0.12"` to Cargo.toml dependencies
   - Change `Vec<String>` warnings to use SmallVec or LRU:
   ```rust
   const MAX_WARNINGS: usize = 100;
   if warnings.len() >= MAX_WARNINGS {
       // Use LRU cache to evict oldest warnings
       let mut lru = LruCache::new(NonZeroUsize::new(MAX_WARNINGS).unwrap());
       for w in warnings.drain(..) {
           lru.put(w.clone(), ());
       }
       warnings = lru.iter().map(|(k, _)| k.clone()).collect();
   }
   warnings.push(new_warning);
   ```
3. **Add error injection tests** in `tests/pipeline_error_injection_test.rs`:
   - Corrupt packets: invalid checksums, wrong lengths, truncated headers
   - Invalid protocol numbers: protocol 255 (unknown)
   - Decoder failures: Create mock decoder that returns errors
   - Verify errors logged but pipeline doesn't panic
   - Test error recovery: After error, next valid packet processes correctly
   - Test cases:
     - Malformed IP header
     - Truncated TCP header
     - Unknown protocol in IP header
     - Decoder throws DecodeError
     - TLS decryption failure
     - All should result in fallback event or error event, never panic
4. **Add property test** for pipeline robustness in `tests/pipeline_property_test.rs`:
   ```rust
   proptest! {
       #[test]
       fn pipeline_never_panics_with_arbitrary_packets(
           packets in prop::collection::vec(
               prop::collection::vec(any::<u8>(), 0..2000),
               0..100
           )
       ) {
           let mut pipeline = PipelineCore::new(None, DecoderRegistry::default());
           for packet_data in packets {
               // Should never panic regardless of input
               let _ = pipeline.process_packet(1, 0, &packet_data, "test");
           }
       }
   }
   ```
5. **Add stress test** (benchmark) in `benches/pipeline_stress_bench.rs`:
   ```rust
   fn benchmark_pipeline_with_errors(c: &mut Criterion) {
       c.bench_function("pipeline_100k_packets_10pct_errors", |b| {
           b.iter(|| {
               let mut pipeline = PipelineCore::new(None, DecoderRegistry::default());
               for i in 0..100_000 {
                   let packet = if i % 10 == 0 {
                       create_corrupt_packet() // 10% error rate
                   } else {
                       create_valid_packet()
                   };
                   let _ = pipeline.process_packet(1, i * 1000, &packet, "bench");
               }
           });
       });
   }
   ```

**Alternatives ruled out:**
- Removing warnings field entirely: Rejected—valuable for debugging protocol issues, limit capacity instead of removing
- Panicking on warning overflow: Rejected—hot path must never panic, dropping old warnings is safer

**Pre-mortem risks:**
- LRU cache adds allocations in hot path: Mitigate by using SmallVec for small counts (<10 warnings), only LRU when approaching limit
- Error injection could surface bugs in decoders: Good—that's the point of testing, fix them
- Stress test might timeout in CI: Make it a benchmark (benches/), not a test (tests/), run separately

**Segment-specific commands:**
- Build: `cargo build -p prb-pcap`
- Test (targeted): `cargo test -p prb-pcap pipeline_error_injection pipeline_property pipeline_warning_capacity`
- Test (regression): `cargo test -p prb-pcap pipeline`
- Test (full gate): `cargo nextest run -p prb-pcap`
- Benchmark (optional): `cargo bench -p prb-pcap pipeline_stress`

**Exit criteria:**
1. Targeted tests: `pipeline_error_injection` (10 error types handled gracefully), `pipeline_property` (proptest no panics), `pipeline_warning_capacity` (warnings capped at 100)
2. Regression tests: All pipeline tests in `tests/pipeline_tests.rs`, `tests/pipeline_error_tests.rs` pass
3. Full build gate: `cargo build -p prb-pcap` succeeds with zero warnings
4. Full test gate: `cargo nextest run -p prb-pcap` passes
5. Self-review gate: Unwrap replaced with error handling, warning capacity limit implemented, error logging added
6. Scope verification gate: Only modified `pipeline_core.rs` and test files; added lru dependency to Cargo.toml

**Risk factor:** 8/10

**Estimated complexity:** Medium

**Commit message:** `fix(pcap-pipeline): Add error handling and warning capacity limit in hot path`

---

## Segment 7: AI HTTP Mocking
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Increase explain.rs coverage from 36.73% to 75%+ with HTTP-level mocking using wiremock.

**Depends on:** None (independent)

**Issues addressed:** AI Streaming and Error Handling

**Cycle budget:** 10 Low

**Scope:** `crates/prb-ai` HTTP mocking tests (no production code changes)

**Key files and context:**
- `/Users/psauer/probe/crates/prb-ai/src/explain.rs` (189 lines)
  - Lines 174-183: Streaming interruption, no recovery mechanism
  - Lines 85-88: Empty choices array error path
  - Lines 90-94: Null content handling
  - Uses `async-openai` v0.20 client for OpenAI-compatible APIs
- `/Users/psauer/probe/.claude/plans/coverage-90-hardening-2026-03-10/segments/05-ai-wiremock.md` - Prior planning document explicitly approved wiremock strategy
- Current: 49 tests pass but all skip actual LLM calls, 36.73% coverage in explain.rs indicates HTTP paths untested

**Implementation approach:**
1. **Add wiremock dependency** to `Cargo.toml`:
   ```toml
   [dev-dependencies]
   bytes = { workspace = true }
   wiremock = "0.6.5"
   ```
2. **Create `tests/explain_http_test.rs`** with mock server for OpenAI API `/v1/chat/completions` endpoint
3. **Test success paths** (2 tests):
   - Non-streaming: Mock 200 response with `choices[0].message.content = "explanation"`
   - Streaming: Mock SSE response with multiple chunks:
     ```
     data: {"choices":[{"delta":{"content":"Hello"}}]}\n\n
     data: {"choices":[{"delta":{"content":" world"}}]}\n\n
     data: [DONE]\n\n
     ```
4. **Test error paths** (6 tests):
   - Empty choices array: `{"choices": []}` → AiError::ApiRequest("empty response")
   - Null content: `{"choices":[{"message":{"content":null}}]}` → AiError::ApiRequest("no content")
   - 500 error: HTTP 500 with error JSON → AiError::ApiRequest
   - 429 rate limit: HTTP 429 → AiError::RateLimited (if error type exists, else ApiRequest)
   - Timeout: Mock with long delay, use tokio::time::timeout
   - Stream interruption: Send partial SSE then close connection → AiError::StreamInterrupted
5. **Test request validation** (1 test):
   - Verify request body contains correct structure:
     - `model`: matches config
     - `messages`: array with system + user messages
     - `temperature`: matches config
     - `max_tokens`: matches config
   - Use wiremock body matchers
6. **Add contract test** (optional, `#[ignore]`):
   ```rust
   #[tokio::test]
   #[ignore] // Run with --ignored flag
   async fn test_real_openai_api() {
       let api_key = std::env::var("OPENAI_API_KEY")
           .expect("Set OPENAI_API_KEY for contract tests");
       let config = AiConfig::for_provider(AiProvider::OpenAi).with_api_key(api_key);
       let events = vec![make_test_event()];
       let result = explain_event(&events, 0, &config).await;
       assert!(result.is_ok());
   }
   ```

**Alternatives ruled out:**
- Trait abstraction for HTTP client: Rejected per prior planning—too invasive, adds runtime cost
- Testing only non-streaming: Rejected—streaming is primary use case in TUI, must validate SSE parsing

**Pre-mortem risks:**
- Mock SSE format might diverge from real OpenAI API: Mitigate with optional contract tests against real API
- Wiremock adds ~5MB to dev-dependencies: Acceptable—dev-only dependency, no production impact
- Tests could be brittle to API format changes: Mitigate by centralizing mock response builders in helper functions

**Segment-specific commands:**
- Build: `cargo build -p prb-ai`
- Test (targeted): `cargo test -p prb-ai explain_http`
- Test (regression): `cargo test -p prb-ai`
- Test (full gate): `cargo nextest run -p prb-ai`
- Contract (optional): `cargo test -p prb-ai -- --ignored` (requires $OPENAI_API_KEY or $ANTHROPIC_API_KEY)

**Exit criteria:**
1. Targeted tests: `explain_http_success` (2 tests), `explain_http_errors` (6 tests), `explain_http_validation` (1 test) = 9 new tests pass
2. Regression tests: All existing prb-ai tests in src/*/tests pass (49 existing tests)
3. Full build gate: `cargo build -p prb-ai` succeeds with zero warnings
4. Full test gate: `cargo nextest run -p prb-ai` passes (58 total tests: 49 existing + 9 new)
5. Self-review gate: No production code changes, only test additions, wiremock properly configured
6. Scope verification gate: Only added wiremock to dev-dependencies and created test file, no src/ modifications

**Risk factor:** 5/10

**Estimated complexity:** Low

**Commit message:** `test(ai): Add HTTP mocking for LLM integration with wiremock`

---

## Segment 8: TUI Snapshot Expansion
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Expand TUI snapshot tests from 8 to 30+ covering all critical UI states and input modes.

**Depends on:** None (independent)

**Issues addressed:** TUI Component Testing - visual regression coverage

**Cycle budget:** 12 Low

**Scope:** `crates/prb-tui/tests/tui_snapshots.rs`

**Key files and context:**
- `/Users/psauer/probe/crates/prb-tui/tests/tui_snapshots.rs` - Currently 8 snapshot tests
- `/Users/psauer/probe/crates/prb-tui/tests/buf_helpers.rs` - Test utilities (row_text, find_text, cell_fg)
- Pattern: `insta::assert_snapshot!(render_app(&mut app, width, height))`
- Current snapshots cover: empty state, two events, filtered view, help overlay, filter input mode, panes focused (decode tree, hex dump, timeline)
- Missing: 11 input modes (only 3 covered), 10+ overlays (only 1 covered), 8 panes (only 3 covered), error states

**Implementation approach:**
1. **Add input mode snapshots** in `tui_snapshots.rs` (11 modes, need 8 more):
   - Normal ✓ (exists)
   - Filter ✓ (exists)
   - Help ✓ (exists)
   - GoToEvent (new)
   - Welcome (new)
   - WhichKey (new)
   - CommandPalette (new)
   - PluginManager (new)
   - ExportDialog (new)
   - CaptureConfig (new)
   - ThemeEditor (new)
2. **Add overlay snapshots** (10+ overlays, need 9 more):
   - Metrics overlay
   - FollowStream overlay
   - DiffView overlay
   - SessionInfo overlay
   - TlsKeylogPicker overlay
   - AIFilter overlay (in progress)
   - Plugin manager overlay
   - Export dialog overlay
   - Capture config overlay
   - Theme editor overlay
3. **Add pane focus snapshots** at 2 terminal sizes (8 panes × 2 sizes = 16, need 13 more):
   - EventList ✓ (exists at 80x24)
   - DecodeTree ✓ (exists)
   - HexDump ✓ (exists)
   - Timeline ✓ (exists)
   - Waterfall at 80x24 and 120x40 (new)
   - AI Panel at 80x24 and 120x40 (new)
   - TraceCorrelation at 80x24 and 120x40 (new)
   - ConversationList at 80x24 and 120x40 (new)
   - Also add 120x40 variants for EventList, DecodeTree, HexDump, Timeline (new)
4. **Add error state snapshots** (5 new):
   - Empty store with "No events loaded" message
   - Filter with no matches showing "No events match filter"
   - Failed AI explanation with error message
   - Parse error in filter input
   - Loading state (spinner/progress indicator)
5. **Add edge case snapshots** (4 new):
   - Very long event list (1000+ events, test scrolling UI)
   - Very wide payload (hex dump horizontal scroll indicators)
   - Unicode in event data (verify rendering)
   - Extremely small terminal (40x10, verify graceful degradation)
6. **Organize snapshots** by category using insta settings:
   ```rust
   let mut settings = insta::Settings::clone_current();
   settings.set_snapshot_path("../snapshots/input_modes");
   settings.bind(|| {
       insta::assert_snapshot!("normal_mode", render_app(&mut app, 80, 24));
   });
   ```

**Alternatives ruled out:**
- Snapshot testing with actual terminal output: Rejected—environment-dependent (terminal emulator, fonts), Buffer snapshots more portable
- Testing only happy paths: Rejected—error states are critical UX, users need to see helpful error messages

**Pre-mortem risks:**
- Snapshot churn on minor UI tweaks: Acceptable tradeoff—use `cargo insta review` for efficient review workflow
- Large number of snapshots (30+) adds review burden: Mitigate by organizing in subdirectories and clear naming
- Unicode rendering might differ across platforms: Document expected platform (macOS/Linux) in snapshot metadata

**Segment-specific commands:**
- Build: `cargo build -p prb-tui`
- Test (targeted): `cargo test -p prb-tui tui_snapshots`
- Test (regression): `cargo test -p prb-tui`
- Test (full gate): `cargo nextest run -p prb-tui`
- Review snapshots: `cargo insta review` (after snapshots update)

**Exit criteria:**
1. Targeted tests: `tui_snapshots` (30+ tests pass: 8 existing + 22+ new), all input modes covered, all overlays covered
2. Regression tests: All existing TUI tests pass (600+ tests)
3. Full build gate: `cargo build -p prb-tui` succeeds with zero warnings
4. Full test gate: `cargo nextest run -p prb-tui` passes
5. Self-review gate: Snapshots organized by category in subdirectories, no duplicate coverage, clear naming convention
6. Scope verification gate: Only `tui_snapshots.rs` and snapshot files modified, no src/ changes

**Risk factor:** 4/10

**Estimated complexity:** Low

**Commit message:** `test(tui): Expand snapshot coverage to 30+ UI states and overlays`

---

## Segment 9: TUI Interactive Testing
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Add async, mouse, and resize testing for TUI components to cover interaction flows.

**Depends on:** None (independent)

**Issues addressed:** TUI Component Testing - interaction flows, async event handling

**Cycle budget:** 15 Medium

**Scope:** `crates/prb-tui` async/interaction tests (new test files)

**Key files and context:**
- `/Users/psauer/probe/crates/prb-tui/src/app.rs` - State machine with 11 input modes, mouse support (lines 500-600), resize handling
- `/Users/psauer/probe/crates/prb-tui/src/live.rs` - Async live capture with tokio channels
- Existing: `test_handle_key()` method for synchronous key simulation
- Missing: Async tests for live capture, mouse interaction tests, resize tests, property tests for navigation

**Implementation approach:**
1. **Add async live capture tests** in `tests/async_capture_test.rs`:
   ```rust
   #[tokio::test]
   async fn test_live_capture_event_stream() {
       let (tx, rx) = tokio::sync::mpsc::channel(100);
       let mut app = App::new_live(rx);

       tx.send(event1).await.unwrap();
       tokio::time::sleep(Duration::from_millis(10)).await;

       assert_eq!(app.state.store.len(), 1);
       assert_eq!(app.state.store.get(0), Some(&event1));
   }

   #[tokio::test]
   async fn test_ring_buffer_overflow() {
       let (tx, rx) = tokio::sync::mpsc::channel(100);
       let mut app = App::new_live_with_ring_buffer(rx, 10);

       for i in 0..20 {
           tx.send(make_event(i)).await.unwrap();
       }
       tokio::time::sleep(Duration::from_millis(50)).await;

       assert_eq!(app.state.store.len(), 10); // Ring buffer capacity
       // Verify oldest events dropped, newest 10 retained
   }
   ```
2. **Add mouse interaction tests** in `tests/mouse_test.rs`:
   - Pane focus by click: Click on decode tree pane area, verify focus changes from EventList to DecodeTree
   - Resize drag: Simulate drag on split border, verify split percentages update (e.g., 50/50 → 60/40)
   - Scroll with mouse wheel: Send ScrollUp/ScrollDown events, verify pane scroll offsets change
   ```rust
   #[test]
   fn test_mouse_pane_focus() {
       let mut app = setup_app_with_panes();
       assert_eq!(app.focus, PaneId::EventList);

       // Click on decode tree pane (assume rect at x=40, y=10)
       app.handle_mouse_event(MouseEvent {
           kind: MouseEventKind::Down(MouseButton::Left),
           column: 40,
           row: 10,
           modifiers: KeyModifiers::empty(),
       });

       assert_eq!(app.focus, PaneId::DecodeTree);
   }
   ```
3. **Add terminal resize tests** in `tests/resize_test.rs`:
   - Render at 80×24, resize to 120×40, verify:
     - Selection index preserved
     - Layout recalculated (pane_rects updated)
     - No panic or visual glitches
   - Test resize during filter input mode (edge case - ensure input not lost)
   - Test resize with zoomed pane (should maintain zoom state)
4. **Add property tests** for navigation in `tests/navigation_property_test.rs`:
   ```rust
   use proptest::prelude::*;

   proptest! {
       #[test]
       fn navigation_selection_always_valid(
           keys in prop::collection::vec(
               prop::sample::select(vec![KeyCode::Up, KeyCode::Down, KeyCode::PageUp, KeyCode::PageDown]),
               0..100
           ),
           event_count in 0usize..1000
       ) {
           let events = (0..event_count).map(|i| make_event(i)).collect();
           let mut app = App::new(EventStore::new(events), None, None);

           for key in keys {
               app.test_handle_key(KeyEvent::new(key, KeyModifiers::NONE));

               // Invariants:
               if event_count > 0 {
                   assert!(app.state.selected_event.is_some());
                   let idx = app.state.selected_event.unwrap();
                   assert!(idx < event_count); // Never out of bounds
               }
           }
       }
   }
   ```
5. **Add keyboard-only navigation test** (accessibility) in `tests/accessibility_test.rs`:
   - Navigate through all 8 panes with Tab key
   - Open and close all overlays with keyboard shortcuts (?, c, e, etc.)
   - Verify no mouse-only features (all actions keyboard-accessible)
   - Test with screen reader simulation (optional: verify ARIA-like hints in status bar)

**Alternatives ruled out:**
- Mocking at TTY level: Rejected—too complex, TestBackend + event simulation sufficient
- Only synchronous tests: Rejected—async is production reality with live capture, must validate tokio integration

**Pre-mortem risks:**
- Async tests could be flaky on timing: Mitigate by using deterministic event ordering, avoid real-time sleeps where possible
- Mouse tests depend on exact layout calculation: Use specific known terminal sizes (80×24, 120×40) and test with those
- Property tests might be slow with 100-key sequences: Acceptable—property tests should be thorough, can run with `--release` if needed

**Segment-specific commands:**
- Build: `cargo build -p prb-tui`
- Test (targeted): `cargo test -p prb-tui async_capture mouse_interaction resize navigation_property`
- Test (regression): `cargo test -p prb-tui`
- Test (full gate): `cargo nextest run -p prb-tui`

**Exit criteria:**
1. Targeted tests: `async_capture` (5 tests for event stream, ring buffer, state transitions), `mouse_interaction` (3 tests for click, drag, scroll), `resize` (3 tests for layout preservation), `navigation_property` (proptest passes) = 12+ new tests pass
2. Regression tests: All existing TUI tests pass (600+ tests)
3. Full build gate: `cargo build -p prb-tui` succeeds with zero warnings
4. Full test gate: `cargo nextest run -p prb-tui` passes (612+ total tests)
5. Self-review gate: Async tests use deterministic timing with tokio::time::pause where possible, property tests bounded to prevent CI timeout
6. Scope verification gate: Only test files added in `tests/` directory, no src/ changes

**Risk factor:** 4/10

**Estimated complexity:** Medium

**Commit message:** `test(tui): Add async, mouse, and resize interaction tests`

---

## Execution Log

| Segment | Status | Started | Completed | Notes |
|---------|--------|---------|-----------|-------|
| 1 | | | | |
| 2 | | | | |
| 3 | | | | |
| 4 | | | | |
| 5 | | | | |
| 6 | | | | |
| 7 | | | | |
| 8 | | | | |
| 9 | | | | |

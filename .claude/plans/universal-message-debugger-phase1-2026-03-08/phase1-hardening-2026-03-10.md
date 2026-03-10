# Phase 1 Hardening Plan — Bug Fixes, Missing Implementations, and Test Overhaul

**Goal**: Address every known bug, TODO, stub, and test gap in one sweep. Bring test
quality to production-grade: every public API has unit tests, every crate has integration
tests with realistic data, every end-to-end path has coverage, and edge cases / error
paths are explicitly exercised.

**Scope**: All 8 crates in the workspace. ~40 discrete changes organized into 6 work
streams that can be executed mostly sequentially (some parallelize within a stream).

---

## WS-1: Hard Bugs (must fix first — other work depends on correctness)

### WS-1.1: TCP stream events use `Timestamp::now()` instead of capture time

**File**: `crates/prb-pcap/src/pipeline.rs:274`
**Bug**: `create_debug_event_from_stream` calls `Timestamp::now()` instead of propagating
the capture timestamp. UDP events correctly use `normalized.timestamp_us`.

**Fix**:
1. Add `timestamp_us: u64` field to `ReassembledStream` (`crates/prb-pcap/src/tcp.rs`).
   Set it from `NormalizedPacket.timestamp_us` during reassembly — use the timestamp of
   the first segment for the stream.
2. Propagate through `DecryptedStream` (`crates/prb-pcap/src/tls/mod.rs`) — add matching
   `timestamp_us: u64` field, copy from `ReassembledStream`.
3. In `pipeline.rs:create_debug_event_from_stream`, use
   `Timestamp::from_nanos(stream.timestamp_us * 1000)` instead of `Timestamp::now()`.

**Also fix in decoders**: `GrpcDecoder`, `ZmqDecoder`, and `DdsDecoder` all use
`Timestamp::now()` for events created from `decode_stream()`. These decoders receive data
from already-reassembled TCP streams and should accept a timestamp via `DecodeContext`.
- Add `pub timestamp: Option<Timestamp>` field to `DecodeContext` in `prb-core`.
- In each decoder's event builder, use `ctx.timestamp.unwrap_or_else(Timestamp::now)`.

### WS-1.2: HPACK `parse_integer` offset bug

**File**: `crates/prb-grpc/src/h2.rs:268-300`
**Bug**: `parse_integer` returns `(value)` but all callers do `offset += 1`, which is
wrong for multi-byte integers (values >= 128). Also wrong for the indexed header path
(0x80 prefix) since the mask is 7 bits.

**Fix**: Change `parse_integer` to return `(value, bytes_consumed)`:
```rust
fn parse_integer(&self, data: &[u8], n: u8) -> Result<(usize, usize), GrpcError> {
    // ... existing logic ...
    Ok((value, offset))  // offset = number of bytes consumed
}
```
Update all call sites in `parse_hpack_headers` to advance by the returned byte count
instead of hardcoded `1`.

### WS-1.3: Encrypted vs. decrypted transport kind is identical

**File**: `crates/prb-pcap/src/pipeline.rs:265-271`
**Bug**: Both branches of `if stream.encrypted` return `TransportKind::RawTcp`.

**Fix**: The `encrypted` field on `DecryptedStream` is `true` when decryption failed
(data is still encrypted). When `encrypted == false`, the data was successfully
decrypted. The transport kind should still be `RawTcp` in both cases at this pipeline
stage (protocol decoders classify later), but add metadata to distinguish:
```rust
.metadata("pcap.tls_decrypted", (!stream.encrypted).to_string())
```
Delete the dead `if/else` and just use `TransportKind::RawTcp` unconditionally.

### WS-1.4: DDS SEDP discovery registers wrong GUID

**File**: `crates/prb-dds/src/decoder.rs:111-119`
**Bug**: `process_discovery_data` receives `writer_guid` which is the SEDP writer's GUID
(e.g., `[prefix]:000003C2`). It should extract the *advertised* writer's GUID from the
CDR payload and register *that* instead.

**Fix**:
1. In `process_discovery_data`, parse the CDR parameter list from the SEDP payload:
   - Read encapsulation header (2 bytes: `{0x00, 0x01}` = LE CDR, `{0x00, 0x00}` = BE CDR)
   - Skip 2 bytes options
   - Walk PID/length pairs:
     - `PID_TOPIC_NAME (0x0005)`: CDR string (4-byte length + UTF-8 + null + padding)
     - `PID_TYPE_NAME (0x0007)`: CDR string
     - `PID_ENDPOINT_GUID (0x005A)` or `PID_KEY_HASH (0x0070)`: 16-byte GUID
     - `PID_SENTINEL (0x0001)`: stop
   - Extract topic_name, type_name, and the advertised endpoint's GUID.
2. Register under the advertised GUID, not the SEDP writer GUID.
3. Remove the heuristic `extract_cdr_string` method entirely.

---

## WS-2: Incomplete Implementations

### WS-2.1: DSB-to-TlsKeyLog bridge

**Files**: `crates/prb-pcap/src/reader.rs`, `crates/prb-pcap/src/pipeline.rs:106-122`

**Current state**: `TlsKeyStore` stores DSB keys as `HashMap<Vec<u8>, Vec<u8>>` (client_random
→ master_secret). `TlsKeyLog` has `merge_dsb_keys(&[u8])` which expects raw SSLKEYLOGFILE
text. The pipeline detects DSB keys but creates an empty `TlsKeyLog`.

**Fix**: Add an `iter()` method to `TlsKeyStore`:
```rust
impl TlsKeyStore {
    pub fn iter(&self) -> impl Iterator<Item = (&[u8], &[u8])> {
        self.keys.iter().map(|(k, v)| (k.as_slice(), v.as_slice()))
    }
}
```
In `pipeline.rs`, convert `TlsKeyStore` entries to `TlsKeyLog`:
```rust
let mut keylog = TlsKeyLog::new();
for (client_random, master_secret) in embedded_keys.iter() {
    keylog.insert(
        client_random.to_vec(),
        KeyMaterial::MasterSecret(master_secret.to_vec()),
    );
}
TlsStreamProcessor::with_keylog(keylog)
```
Remove the TODO comment.

### WS-2.2: HPACK static table completion + literal-with-indexing support

**File**: `crates/prb-grpc/src/h2.rs:203-266, 302-317`

1. Expand `static_table_lookup` to cover all 61 entries from RFC 7541 Appendix A.
2. Handle literal-with-incremental-indexing (0x40 prefix) — same parse as literal-without
   (0x00) but the entry should be noted (we don't need a dynamic table for offline analysis,
   but we must consume the bytes correctly to avoid corrupting the parse state).
3. Handle literal-never-indexed (0x10 prefix) — same byte layout as 0x00.
4. Handle dynamic table size update (0x20 prefix) — consume the integer and ignore.
5. Remove `h2-sans-io` from `Cargo.toml` since it's unused.

### WS-2.3: HTTP/2 CONTINUATION frame support

**File**: `crates/prb-grpc/src/h2.rs:136-194`

HEADERS frames can be followed by CONTINUATION frames (type 0x09) when the header block
is too large for a single frame. Without this, fragmented headers silently produce
partial/empty header maps.

**Fix**: In the frame parser:
1. Track whether we're in a "header block continuation" state (HEADERS without END_HEADERS
   flag `0x04` set).
2. Buffer header block fragments across HEADERS + CONTINUATION frames.
3. Only call `parse_hpack_headers` once we see END_HEADERS.

### WS-2.4: ZMQ single-frame PUB/SUB topic extraction

**File**: `crates/prb-zmq/src/decoder.rs:126-132`
**Bug**: When `frames.len() == 1` for PUB/SUB, the topic is not extracted.

**Fix**:
```rust
let (topic, payload_frames) = if socket_type == "PUB" || socket_type == "SUB" {
    if frames.len() > 1 {
        let topic = String::from_utf8_lossy(&frames[0]).to_string();
        (Some(topic), &frames[1..])
    } else if frames.len() == 1 {
        let topic = String::from_utf8_lossy(&frames[0]).to_string();
        (Some(topic), &frames[0..0])  // topic only, no payload
    } else {
        (None, &frames[..])
    }
} else {
    (None, &frames[..])
};
```

### WS-2.5: CLI magic-bytes format detection

**File**: `crates/prb-cli/src/commands/ingest.rs:16-37`

Replace extension-based dispatch with magic-bytes detection:
```rust
fn detect_format(path: &Path) -> Result<InputFormat> {
    let mut file = File::open(path)?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;

    match &magic {
        b"RTPS" => unreachable!(), // not a file format
        [0x0a, 0x0d, 0x0d, 0x0a] => Ok(InputFormat::Pcapng),
        [0xa1, 0xb2, 0xc3, 0xd4] | [0xd4, 0xc3, 0xb2, 0xa1] => Ok(InputFormat::Pcap),
        [b'{', ..] | [b'[', ..] => Ok(InputFormat::Json),   // JSON starts with { or [
        _ => {
            // Fall back to extension
            match path.extension().and_then(|e| e.to_str()) {
                Some("json") => Ok(InputFormat::Json),
                Some("pcap") => Ok(InputFormat::Pcap),
                Some("pcapng") => Ok(InputFormat::Pcapng),
                _ => bail!("Cannot detect format for {}", path.display()),
            }
        }
    }
}
```

### WS-2.6: Direction inference for all decoders

All three protocol decoders hardcode `Direction::Inbound`. Fix:
- `GrpcDecoder`: Odd stream IDs are client-initiated (request = Outbound from client
  perspective). Even = server push. Response DATA/trailers = Inbound.
  Actually, simpler: request headers have `:method` → Outbound; response/trailers → Inbound.
  The decoder already distinguishes request vs response headers in stream state.
- `ZmqDecoder`: Use `as_server` from greeting. If `as_server == true`, messages from this
  side are `Outbound`. Store in decoder state after greeting.
- `DdsDecoder`: DDS is pub/sub multicast; direction doesn't map well. Keep `Inbound` but
  document why. No change needed.

### WS-2.7: gRPC deflate compression (use ZlibDecoder, not DeflateDecoder)

**File**: `crates/prb-grpc/src/lpm.rs:126-133`

gRPC "deflate" encoding is actually zlib (RFC 1950), not raw deflate (RFC 1951).
Change `flate2::read::DeflateDecoder` to `flate2::read::ZlibDecoder`.

### WS-2.8: Remove dead code and unused dependencies

- Remove `h2-sans-io` from `crates/prb-grpc/Cargo.toml`
- Remove `#[allow(dead_code)]` on `LpmParser::flush` — either use it or delete it
- Remove `ZmqError::IncompleteData` variant (unused)
- Remove `#[allow(dead_code)]` on DDS discovery constants/methods and either use them
  in tests or delete them
- Remove `#[allow(dead_code)]` on `MAX_BUFFER_SIZE` / `MAX_GAP_SIZE` in `tcp.rs`

---

## WS-3: Unit Test Gaps (per-module, test every public function)

### WS-3.1: `prb-grpc` unit tests

**h2.rs**:
- `test_h2_multi_byte_integer`: Header name/value with length >= 128 (exercises multi-byte
  HPACK integer decoding after WS-1.2 fix)
- `test_h2_indexed_header_static_table`: Each of the 61 static table entries
- `test_h2_continuation_frame`: HEADERS split across HEADERS + CONTINUATION (after WS-2.3)
- `test_h2_unknown_frame_skip`: Unknown frame types are skipped without error
- `test_h2_padded_data_frame`: DATA frame with PADDED flag set
- `test_h2_partial_frame_buffering`: Frame split across two `process()` calls
- `test_h2_empty_headers_frame`: Zero-length HEADERS payload

**lpm.rs**:
- `test_lpm_compressed_gzip_roundtrip`: Compress with flate2, then parse
- `test_lpm_compressed_zlib_roundtrip`: Same for zlib/deflate (after WS-2.7)
- `test_lpm_zero_length_message`: Valid LPM with 0-byte payload
- `test_lpm_max_message_size`: Message approaching u32::MAX length (should handle gracefully)

**decoder.rs**:
- `test_grpc_direction_inference`: Request → Outbound, response → Inbound (after WS-2.6)
- `test_grpc_correlation_by_stream_id`: Same stream ID across request/response events
- `test_grpc_metadata_extraction`: `:authority`, `:path`, `grpc-encoding` appear in event metadata

### WS-3.2: `prb-zmq` unit tests

**parser.rs**:
- `test_zmtp_incremental_greeting`: Feed greeting 1 byte at a time, assert greeting event
  only after byte 64
- `test_zmtp_frame_boundary_split`: Message frame header in one feed, body in next
- `test_zmtp_max_multipart_limit`: >1000 frames triggers `TooManyFrames` error
- `test_zmtp_long_command_frame`: Command with 8-byte length encoding
- `test_zmtp_degraded_message_extraction`: Mid-stream data → degraded mode → parse valid
  frames and assert message content
- `test_zmtp_ready_empty_properties`: READY with zero properties

**decoder.rs**:
- `test_zmq_single_frame_pubsub_topic`: Single-frame PUB message extracts topic (after WS-2.4)
- `test_zmq_direction_from_greeting`: `as_server` flag influences direction (after WS-2.6)
- `test_zmq_connection_id_from_context`: Connection ID generated from src/dst addresses
- `test_zmq_degradation_warning_present`: Events in degraded mode carry warning string

### WS-3.3: `prb-dds` unit tests

**rtps_parser.rs**:
- `test_rtps_bad_magic`: Non-RTPS data returns `InvalidMagic` error
- `test_rtps_truncated_header`: <20 bytes returns appropriate error
- `test_rtps_data_frag_submessage`: DATA_FRAG is silently skipped (not an error)
- `test_rtps_multiple_submessages`: Message with INFO_TS + DATA + HEARTBEAT + DATA
- `test_rtps_big_endian_submessage`: Submessage with E-flag = 0 (big-endian)

**discovery.rs** (after WS-1.4 CDR parsing):
- `test_cdr_parameter_list_parse`: Real CDR payload with PID_TOPIC_NAME, PID_TYPE_NAME,
  PID_ENDPOINT_GUID, PID_SENTINEL
- `test_discovery_register_and_lookup`: Register endpoint, look up by GUID, assert topic
- `test_discovery_multiple_endpoints`: Register multiple writers, look up each
- `test_discovery_unknown_guid`: Lookup of unregistered GUID returns None

**decoder.rs**:
- `test_dds_domain_id_boundary`: Port 7399 → None, port 7400 → domain 0, port 7650 → domain 1
- `test_dds_timestamp_propagation`: INFO_TS timestamp appears in DebugEvent (not `now()`)
- `test_dds_sedp_then_user_data`: Full flow: SEDP discovery DATA → user DATA → event has topic name

### WS-3.4: `prb-pcap` pipeline unit tests

- `test_pipeline_timestamp_propagation`: Assert TCP events have capture-time timestamps,
  not wall-clock
- `test_pipeline_dsb_keys_used`: pcapng with DSB → TLS decryption succeeds (after WS-2.1)
- `test_pipeline_tls_metadata`: Decrypted stream events carry `pcap.tls_decrypted=true`
- `test_pipeline_stats_accuracy`: Verify `PipelineStats` counts match expected after
  processing known input

---

## WS-4: Integration Tests (cross-crate, realistic data)

### WS-4.1: Golden-file PCAP test fixtures

Create synthetic but realistic PCAP fixtures in `fixtures/`:

1. **`fixtures/tcp_http.pcap`** — Complete TCP handshake + HTTP GET/response + FIN.
   Generated programmatically in a build script or test setup using raw packet construction.

2. **`fixtures/grpc_unary.pcap`** — TCP + HTTP/2 preface + SETTINGS + HEADERS (gRPC request)
   + DATA (LPM-encoded protobuf) + HEADERS (trailers with grpc-status=0). Synthetic.

3. **`fixtures/zmtp_pubsub.pcap`** — TCP + ZMTP greeting + READY + PUB message with topic.

4. **`fixtures/udp_rtps.pcap`** — UDP datagrams with RTPS headers, INFO_TS, SEDP discovery
   DATA, then user DATA.

These don't need to be from real network captures — they can be constructed byte-by-byte
in test setup functions. But they exercise the full reader → normalizer → reassembly →
decoder path.

### WS-4.2: Cross-crate integration tests

Create `tests/integration/` at the workspace root (or in each crate's `tests/`):

**`prb-pcap/tests/pipeline_grpc_test.rs`**:
- Build synthetic PCAP with gRPC traffic → run through `PcapCaptureAdapter` → feed output
  events through `GrpcDecoder` → assert gRPC method, payload, status extracted

**`prb-pcap/tests/pipeline_zmtp_test.rs`**:
- Synthetic PCAP with ZMTP traffic → pipeline → `ZmqDecoder` → assert topic, frames

**`prb-pcap/tests/pipeline_rtps_test.rs`**:
- Synthetic PCAP with RTPS/UDP → pipeline → `DdsDecoder` → assert domain ID, topic name

**`prb-cli/tests/ingest_mcap_roundtrip.rs`**:
- `prb ingest fixtures/grpc_sample.json -o /tmp/test.mcap` → `prb inspect /tmp/test.mcap`
  → assert events survive roundtrip

**`prb-cli/tests/ingest_pcap_test.rs`**:
- Construct PCAP in test → `prb ingest test.pcap` → parse NDJSON output → assert fields

### WS-4.3: Error / edge-case integration tests

- **Truncated PCAP**: PCAP file truncated mid-packet → graceful error, no panic
- **Empty PCAP**: Valid header, zero packets → empty output
- **Corrupt TCP**: Overlapping sequence numbers → no panic, events still emitted
- **Unknown link type**: PCAP with linktype 999 → error logged, other packets still processed
- **gRPC mid-stream**: Feed DATA without prior HEADERS → HPACK degradation warning, no crash
- **ZMTP mid-stream**: Feed traffic without greeting → degraded mode, messages extracted
- **DDS non-RTPS UDP**: UDP datagrams without RTPS magic → silently skipped
- **Malformed DSB**: pcapng with corrupt DSB block → error logged, packets still processed
- **Large message**: gRPC LPM message approaching 4GB → handled without OOM

---

## WS-5: CLI and End-to-End Tests

### WS-5.1: CLI command coverage

Expand `crates/prb-cli/tests/integration.rs`:

- `test_cli_ingest_pcap_to_mcap`: `.pcap` → `.mcap` → `prb inspect` roundtrip
- `test_cli_ingest_magic_bytes_detection`: Rename `.pcap` to `.bin`, ingest still works
  (after WS-2.5)
- `test_cli_ingest_stdin_ndjson`: Pipe NDJSON to `prb inspect -` via stdin
- `test_cli_ingest_with_tls_keylog`: Construct PCAP + keylog file → assert TLS metadata
- `test_cli_inspect_wire_format_protobuf`: Events with protobuf payloads → `--wire-format`
  decodes fields
- `test_cli_schemas_export_import`: `schemas load` → `schemas export` → verify `.desc` file
- `test_cli_error_messages`: Nonexistent file, bad format → human-readable errors
- `test_cli_empty_input`: Empty JSON file → zero events, exit 0
- `test_cli_large_input_streaming`: Large JSON fixture (>1000 events) → all emitted

### WS-5.2: Snapshot tests (insta)

Add `insta` snapshot tests for stable output formats:

- `test_snapshot_ndjson_grpc_sample`: `prb ingest fixtures/grpc_sample.json` output snapshot
- `test_snapshot_ndjson_multi_transport`: `prb ingest fixtures/multi_transport.json` snapshot
- `test_snapshot_table_format`: `prb inspect --format table` output snapshot
- `test_snapshot_json_format`: `prb inspect --format json` output snapshot

These catch regressions in the serialization format.

---

## WS-6: Cleanup and Polish

### WS-6.1: Fix all clippy warnings

Current warnings (from `cargo clippy --workspace --all-targets`):
- `vec_init_then_push` in test helpers → use `vec![...]` initializer
- `too_many_arguments` on `create_tcp_segment` test helpers → use a builder struct or params struct
- `identity_op` in TLS decrypt test → remove `^ 0x00`
- `expect_fun_call` in pipeline test → use `unwrap_or_else`
- `to_string_in_format_args` in fixture test → remove `.to_string()`
- `inconsistent_digit_grouping` in DDS test → fix grouping
- `unused_variables` (`events_rst`, `decoder`) → prefix with `_`
- `unused_mut` → remove `mut`
- `deprecated` `cargo_bin` → replace with `cargo_bin_cmd!` or suppress

### WS-6.2: Remove misleading doc comments

- `h2.rs` module doc says "wraps h2-sans-io" → update to "custom HTTP/2 frame parser"
- `pipeline.rs:274` comment says "Use capture time if available" but uses `Timestamp::now()`
  → fix the code (WS-1.1) then remove the comment

### WS-6.3: Rename misnamed test

- `test_cli_ingest_pcapng_tls` creates a legacy PCAP, not pcapng → rename to
  `test_cli_ingest_pcap_tls` or fix to actually use pcapng

---

## Execution Order

1. **WS-1** (hard bugs) — do first, all other work depends on correctness
2. **WS-2** (incomplete impls) — unblocks realistic integration tests
3. **WS-6** (cleanup) — easier to write clean tests on clean code
4. **WS-3** (unit tests) — test each fix in isolation
5. **WS-4** (integration tests) — test cross-crate paths
6. **WS-5** (CLI/E2E tests) — final validation layer

Within each stream, items are independent and can be parallelized.

**Estimated test count after completion**: ~300+ tests (currently 193).
**Expected coverage**: >80% line coverage on all library crates.

---

## Acceptance Criteria

- [ ] `cargo build --workspace` — zero errors, zero warnings
- [ ] `cargo clippy --workspace --all-targets` — zero warnings
- [ ] `cargo test --workspace` — all tests pass
- [ ] Every public function in every crate has at least one direct test
- [ ] Every error variant is exercised by at least one test
- [ ] Cross-crate integration tests cover: PCAP→gRPC, PCAP→ZMQ, PCAP→DDS, JSON→MCAP→inspect
- [ ] No `#[allow(dead_code)]` except on intentionally-reserved API surface
- [ ] No `TODO` or `FIXME` comments remain
- [ ] No `Timestamp::now()` in any code path that handles captured (non-live) data
- [ ] DSB keys from pcapng are actually used for TLS decryption
- [ ] DDS discovery correctly resolves topic names from SEDP data

# Cross-Plan Verification Report

**Plans verified:**
- `universal-message-debugger-phase1-2026-03-08.md` (top-level)
- `subsection-1-foundation-core-model.md` + segments 01–03
- `subsection-2-storage-schema-engine.md` + segments 01–04
- `subsection-3-network-capture-pipeline.md` + segments 01–05
- `subsection-4-protocol-decoders.md` + segments 01–03
- `subsection-5-analysis-replay.md` + segments 01–06

**Upstream authority:** Subsection 1 defines canonical types/traits; earlier subsections are upstream.

**Verdict:** INCONSISTENCIES FOUND

---

## Inconsistencies

### [Category 1]: Path inconsistency — Subsection 2 uses short crate directory names

- **Upstream** (`subsection-1-foundation-core-model.md`, `issue-S1-3-workspace-structure.md`): Defines `crates/prb-core/`, `crates/prb-fixture/`, `crates/prb-cli/`, `crates/prb-storage/`, `crates/prb-schema/`, `crates/prb-decode/`, etc. All crates use `prb-{domain}` directory naming.
- **Downstream** (`subsection-2-storage-schema-engine/segments/*.md`, `subsection-2-storage-schema-engine.md`): Uses `crates/core/`, `crates/cli/`, `crates/storage/`, `crates/schema/`, `crates/decode/` (no `prb-` prefix).
- **Impact:** Build fails; `cargo build -p prb-storage` expects a crate named `prb-storage`, but if the directory is `crates/storage/` the package name in Cargo.toml would typically be `prb-storage`. The inconsistency is in the *directory* paths referenced in Scope and Key Files. Subsection 2 segments say "Create `crates/storage/`" while Subsection 1 defines `crates/prb-storage/`. A builder following Subsection 2 would create the wrong directory structure.
- **Recommended fix:** Update all Subsection 2 segment and plan files to use `crates/prb-storage/`, `crates/prb-schema/`, `crates/prb-decode/`, `crates/prb-core/`, `crates/prb-cli/` consistently.
- **Auto-correctable:** Yes (search-replace)

---

### [Category 1]: Path inconsistency — Protocol decoder crate names differ from workspace layout

- **Upstream** (`subsection-1-foundation-core-model.md`, `issue-S1-3-workspace-structure.md`): Defines `prb-grpc/`, `prb-zmq/`, `prb-dds/` for Subsection 4.
- **Downstream** (`subsection-4-protocol-decoders/segments/*.md`): Uses `prb-protocol-grpc`, `prb-protocol-zmtp`, `prb-protocol-dds` in build commands and scope.
- **Impact:** Build commands `cargo build -p prb-protocol-grpc` fail if the workspace defines `prb-grpc`. Package names must match Cargo.toml.
- **Recommended fix:** Either (a) update Subsection 1 workspace structure to use `prb-protocol-grpc`, `prb-protocol-zmtp`, `prb-protocol-dds`, or (b) update Subsection 4 segments to use `prb-grpc`, `prb-zmq`, `prb-dds`. The `prb-protocol-*` naming is more descriptive; recommend updating Subsection 1.
- **Auto-correctable:** Yes (coordinate both plans)

---

### [Category 2]: Interface contract — CorrelationStrategy trait signature mismatch

- **Upstream** (`subsection-1-foundation-core-model/segments/02-traits-fixture-adapter.md`, `issue-S1-2-sync-async-design.md`):
  ```rust
  pub trait CorrelationStrategy {
      fn transport(&self) -> TransportKind;
      fn correlate<'a>(&self, events: &'a [DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError>;
  }
  ```
- **Downstream** (`subsection-5-analysis-replay/segments/01-correlation-core.md`, `subsection-5-analysis-replay.md`):
  ```rust
  pub trait CorrelationStrategy: Send + Sync {
      fn name(&self) -> &str;
      fn matches(&self, event: &DebugEvent) -> bool;
      fn correlation_key(&self, event: &DebugEvent) -> Option<CorrelationKey>;
  }
  ```
- **Impact:** Subsection 5's correlation engine cannot implement strategies against Subsection 1's trait. The designs are incompatible: Subsection 1 uses batch `correlate(events) -> Vec<Flow>`, Subsection 5 uses per-event `correlation_key(event) -> Option<CorrelationKey>` with engine-side flow assembly.
- **Recommended fix:** Reconcile the trait design. Subsection 5's per-event key extraction is more flexible for streaming and on-demand correlation. Update Subsection 1's `traits.rs` to the Subsection 5 signature and remove or repurpose `correlate()`. Add `transport()` for dispatch if needed. Ensure `Flow` and `CorrelationKey` types are defined in prb-core.
- **Auto-correctable:** No (design decision required)

---

### [Category 2]: Interface contract — SchemaResolver trait signature mismatch

- **Upstream** (`subsection-1-foundation-core-model/segments/02-traits-fixture-adapter.md`):
  ```rust
  pub trait SchemaResolver {
      fn resolve(&self, schema_name: &str) -> Result<Option<ResolvedSchema>, CoreError>;
      fn list_schemas(&self) -> Vec<String>;
  }
  ```
- **Downstream** (`subsection-2-storage-schema-engine/segments/02-protobuf-schema-registry.md`): "Expected signature: `fn resolve(&self, type_name: &str) -> Option<SchemaInfo>` where SchemaInfo contains encoding, name, and raw descriptor bytes."
- **Impact:** Subsection 2's SchemaRegistry cannot implement Subsection 1's trait: return type differs (`Result<Option<ResolvedSchema>>` vs `Option<SchemaInfo>`), and type names differ (`ResolvedSchema` vs `SchemaInfo`).
- **Recommended fix:** Align on one contract. Subsection 1's `Result`-based signature is better for error propagation. Update Subsection 2 to implement `Result<Option<ResolvedSchema>, CoreError>` and ensure `ResolvedSchema` in prb-core includes encoding, name, and descriptor bytes. Subsection 2's "SchemaInfo" should map to `ResolvedSchema`.
- **Auto-correctable:** No (requires type definition alignment)

---

### [Category 2]: Interface contract — CaptureAdapter trait signature mismatch

- **Upstream** (`subsection-1-foundation-core-model/segments/02-traits-fixture-adapter.md`):
  ```rust
  pub trait CaptureAdapter {
      fn name(&self) -> &str;
      fn ingest(&mut self) -> Box<dyn Iterator<Item = Result<DebugEvent, CoreError>> + '_>;
  }
  ```
- **Downstream** (`subsection-3-network-capture-pipeline/segments/05-pipeline-integration.md`): "The `CaptureAdapter` trait ... defines the interface: `fn ingest(&self, source: &Path, options: IngestOptions) -> Result<Session>`."
- **Impact:** PcapCaptureAdapter cannot implement the upstream trait. Upstream has no `source` or `options`; it returns an iterator. Downstream expects path-based ingest returning a Session.
- **Recommended fix:** Either (a) extend CaptureAdapter with an overloaded/optional `ingest_from_path` for path-based adapters, or (b) make the trait generic over a config type. The JSON fixture adapter reads from a path set at construction; the PCAP adapter needs runtime path + options. Recommend adding `fn ingest_from_path(&mut self, source: &Path, options: IngestOptions) -> Result<impl Iterator<Item = Result<DebugEvent, CoreError>>>` or a similar extension. Alternatively, have both adapters take path/options at construction and keep `ingest()` parameterless.
- **Auto-correctable:** No (design decision required)

---

### [Category 3]: Dependency assumption — Subsection 2 assumes non-existent paths

- **Upstream** (Subsection 1 exit criteria): Creates `crates/prb-core/`, `crates/prb-cli/`, `crates/prb-fixture/`.
- **Downstream** (`subsection-2-storage-schema-engine/segments/01-mcap-session-storage.md`): "Subsection 1 produces: `crates/core/src/event.rs` ... `crates/cli/src/main.rs`"
- **Impact:** Builder cannot find referenced files; wrong paths in handoff context.
- **Recommended fix:** Update Subsection 2 segments to reference `crates/prb-core/src/event.rs`, `crates/prb-cli/src/main.rs`, etc.
- **Auto-correctable:** Yes

---

### [Category 4]: Build command consistency — Protocol decoder package names

- **Upstream** (Subsection 1 workspace): Defines `prb-grpc`, `prb-zmq`, `prb-dds`.
- **Downstream** (Subsection 4 segments): Uses `-p prb-protocol-grpc`, `-p prb-protocol-zmtp`, `-p prb-protocol-dds`.
- **Impact:** `cargo build -p prb-protocol-grpc` fails if the workspace only has `prb-grpc`.
- **Recommended fix:** Align package names. Prefer `prb-protocol-grpc` etc. and update Subsection 1 workspace structure.
- **Auto-correctable:** Yes (coordinate both)

---

### [Category 5]: Scope overlap — CLI crate modified by multiple subsections

- **Files:** `crates/prb-cli/` (or `crates/cli/` in Subsection 2)
- **Modified by:** Subsection 1 (create), Subsection 2 (--output, schemas, inspect MCAP, decode, wire-format), Subsection 3 (ingest PCAP), Subsection 4 (inspect decoded), Subsection 5 (flows, replay)
- **Impact:** Expected overlap. No conflict if changes are additive and ordered by subsection. Ensure Subsection 2 uses correct path `crates/prb-cli/`.
- **Recommended fix:** Document CLI as the integration point. Fix path to `prb-cli` in Subsection 2.
- **Auto-correctable:** Yes (path fix only)

---

### [Category 6]: Top-level plan alignment

- **Verdict:** All five subsections stay within the boundaries described in the parent plan. No scope creep detected.

---

## Reconciliation Actions

| File | Change | Rationale |
|------|--------|-----------|
| `subsection-2-storage-schema-engine/segments/01-mcap-session-storage.md` | Replace `crates/storage/` with `crates/prb-storage/`, `crates/core/` with `crates/prb-core/`, `crates/cli/` with `crates/prb-cli/` | Path consistency with Subsection 1 |
| `subsection-2-storage-schema-engine/segments/02-protobuf-schema-registry.md` | Replace `crates/schema/` with `crates/prb-schema/`, `crates/storage/` with `crates/prb-storage/`, `crates/cli/` with `crates/prb-cli/`, `crates/core/` with `crates/prb-core/` | Path consistency |
| `subsection-2-storage-schema-engine/segments/03-schema-backed-decode.md` | Replace `crates/decode/` with `crates/prb-decode/`, `crates/cli/` with `crates/prb-cli/` | Path consistency |
| `subsection-2-storage-schema-engine/segments/04-wire-format-decode.md` | Replace `crates/decode/` with `crates/prb-decode/`, `crates/cli/` with `crates/prb-cli/` | Path consistency |
| `subsection-2-storage-schema-engine.md` | Same path replacements as above | Path consistency |
| `subsection-1-foundation-core-model/issues/issue-S1-3-workspace-structure.md` | Change `prb-grpc`, `prb-zmq`, `prb-dds` to `prb-protocol-grpc`, `prb-protocol-zmtp`, `prb-protocol-dds` | Align with Subsection 4 crate names |
| `subsection-1-foundation-core-model.md` | Same workspace crate name updates | Align with Subsection 4 |

---

## Summary of Findings

| Category | Inconsistencies | Auto-Correctable |
|----------|-----------------|------------------|
| 1. Path Consistency | 2 (Subsection 2 short paths; protocol crate names) | Yes |
| 2. Interface Contract | 3 (CorrelationStrategy, SchemaResolver, CaptureAdapter) | No (design decisions) |
| 3. Dependency Assumption | 1 (Subsection 2 wrong paths) | Yes |
| 4. Build Command | 1 (protocol package names) | Yes |
| 5. Scope Overlap | 0 (CLI overlap is expected) | N/A |
| 6. Top-Level Alignment | 0 | N/A |

**Critical:** The three interface contract mismatches (CorrelationStrategy, SchemaResolver, CaptureAdapter) require design reconciliation before implementation. Path and build-command fixes can be applied mechanically.

---

## Factual Freshness Verification

**Claims checked:** 28
**Stale/incorrect:** 1
**Verified current:** 27

### Stale Claims

#### pcap-parser: Last release date
- **Plan states:** "last release Aug 2024" (issue-05-pcapng-format.md)
- **Current:** v0.17.0 released 2025-07-25 (latest version)
- **Impact:** None for usage -- v0.17.0 is correct and available. The "last release" date is outdated metadata only.
- **Recommendation:** Update to "last release July 2025" or remove the date; version pin v0.17.0 remains correct.

### Verified Current

**Library versions:**
- `mcap` v0.24.0+ -- current latest 0.24.0 (Dec 2025)
- `prost-reflect` v0.16.3 -- current latest
- `protox` v0.9.1 -- current latest (Dec 2025)
- `pcap-parser` v0.17.0 -- current latest
- `etherparse` v0.19.0 -- current latest (Aug 2025)
- `tls-parser` v0.12.2 -- current latest
- `ring` v0.17+ -- current 0.17.14
- `smoltcp` v0.12+ -- current 0.12.0
- `h2-sans-io` v0.1.0 -- current only version, published 2026-02-15
- `fluke-hpack` v0.3.1 -- current latest (~70K downloads)
- `fluke-h2-parse` v0.1.1 -- current latest
- `rtps-parser` v0.1.1 -- current latest
- `criterion` v0.8.2 -- current latest (Feb 2026)
- `divan` v0.1.21 -- current latest (Apr 2025)
- `tabled` v0.20.0 -- current latest (Jun 2025)
- `thiserror` v2 -- current 2.0.18
- `flate2` -- actively maintained (v1.1.9 Feb 2026)

**Maintenance status:**
- `pcap_tcp_assembler` -- 0 stars, not on crates.io (GitHub only) -- verified
- `zmtp` crate -- dead since 2016 (last update 2016-06-19, 52 downloads/90d) -- verified
- `protobuf-decode` -- does not exist on crates.io -- verified
- `rzmq` v0.5.13 -- updated 2026-02-02 -- verified
- `protobin` v0.6.0 -- actively maintained -- verified

**Protocol/spec:**
- SSLKEYLOGFILE -- RFC 9850 (published Dec 2025) -- verified
- pcapng magic bytes `0x0a0d0d0a` -- verified
- pcap magic bytes `0xa1b2c3d4` / `0xd4c3b2a1` -- verified

**API claims:**
- `etherparse::defrag::IpDefragPool` -- exists in v0.19.0 (docs.rs confirmed)

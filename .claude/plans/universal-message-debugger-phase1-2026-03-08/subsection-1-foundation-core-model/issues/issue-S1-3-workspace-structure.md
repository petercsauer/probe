---
id: "S1-3"
title: "Workspace Crate Structure Undefined"
risk: 2/10
addressed_by_segments: [1]
---
# Issue S1-3: Workspace Crate Structure Undefined

**Core Problem:**
The parent plan mentions "12+ crates in the workspace" but does not name them, define the directory layout, or specify which crates are created in which subsection. Without this, each subsection's deep-plan must independently invent crate names, leading to inconsistency.

**Root Cause:**
The parent plan focused on subsection-level decomposition, not workspace-level structure.

**Proposed Fix:**
Define the full workspace structure now. Subsection 1 creates the first 3 crates; later subsections add theirs.

```
prb/
├── Cargo.toml                  # Virtual workspace manifest
├── Cargo.lock
├── README.md
├── crates/
│   ├── prb-core/               # Subsection 1: types, traits, errors
│   ├── prb-fixture/            # Subsection 1: JSON fixture adapter
│   ├── prb-cli/                # Subsection 1+: CLI binary
│   ├── prb-storage/            # Subsection 2: MCAP read/write
│   ├── prb-schema/             # Subsection 2: protobuf schema subsystem
│   ├── prb-decode/             # Subsection 2: protobuf decode engine
│   ├── prb-pcap/               # Subsection 3: PCAP/pcapng ingest
│   ├── prb-tcp/                # Subsection 3: TCP reassembly
│   ├── prb-tls/                # Subsection 3: TLS decryption
│   ├── prb-grpc/               # Subsection 4: gRPC decoder
│   ├── prb-zmq/                # Subsection 4: ZMQ/ZMTP decoder
│   ├── prb-dds/                # Subsection 4: DDS/RTPS decoder
│   ├── prb-correlation/        # Subsection 5: correlation engine
│   └── prb-replay/             # Subsection 5: replay engine
├── fixtures/                   # Test fixture files
│   └── sample.json
└── tests/                      # Workspace-level integration tests
```

Workspace `Cargo.toml` uses `workspace.dependencies` for all shared dependencies with pinned versions. Crate naming follows `prb-{domain}` convention. All crates use edition 2024.

**Existing Solutions Evaluated:**
- N/A -- internal project structure decision.

**Alternatives Considered:**
- Flat workspace (all crates at root level, no `crates/` directory). Rejected: becomes unwieldy with 14 crates. The `crates/` convention is standard (used by rustc, cargo, tokio, bevy).
- Fewer, larger crates (e.g., single `prb-network` instead of `prb-pcap`, `prb-tcp`, `prb-tls`). Rejected: coarser crate boundaries mean longer recompile times and entangled error types. Fine-grained crates match the subsection decomposition.

**Pre-Mortem -- What Could Go Wrong:**
- 14 crates may be excessive, increasing compile times from inter-crate dependency resolution. Mitigation: Cargo workspaces share a build cache; incremental builds only recompile changed crates.
- Crate names may collide with crates.io packages. Mitigation: the `prb-` prefix is unlikely to collide; these are private crates not published to crates.io.
- The `tests/` directory at workspace root may confuse cargo (it expects `tests/` per-crate). Mitigation: workspace-level integration tests use `[[test]]` entries in the CLI crate's Cargo.toml, not the workspace root.

**Risk Factor:** 2/10

**Evidence for Optimality:**
- External evidence: Cargo workspace best practices (doc.rust-lang.org/cargo/reference/workspaces) recommend virtual manifests for multi-crate projects, `workspace.dependencies` for version deduplication, and the `crates/` directory convention.
- External evidence: Major Rust projects (tokio, bevy, rustc) use the `crates/` layout with domain-specific crate names, validating this structure at scale.

**Blast Radius:**
- Direct changes: root `Cargo.toml`, `crates/prb-core/Cargo.toml`, `crates/prb-fixture/Cargo.toml`, `crates/prb-cli/Cargo.toml`
- Potential ripple: every subsequent subsection adds crates to this structure

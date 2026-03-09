All build and test commands run directly on the host. There is no devcontainer. This is a Rust Cargo workspace project.

---

## Environment

- **Rust toolchain:** edition 2024, resolver 3 (requires Rust ≥ 1.85)
- **Test runner:** `cargo nextest` (faster, better output than `cargo test`)
- **Working directory:** project root (`/Volumes/Local/Development/prb` or wherever the repo is checked out)

## Workspace structure

```
prb/
├── Cargo.toml                  # Virtual workspace manifest
├── crates/
│   ├── prb-core/               # Subsection 1: types, traits, errors
│   ├── prb-fixture/            # Subsection 1: JSON fixture adapter
│   ├── prb-cli/                # Subsection 1+: CLI binary (`prb`)
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
└── fixtures/                   # Test fixture files
```

## Build commands

```bash
# Build a specific crate
cargo build -p prb-core

# Build the full workspace
cargo build --workspace

# Release build
cargo build --release -p prb-cli
```

## Test commands

```bash
# Run tests for a specific crate (preferred)
cargo nextest run -p prb-core

# Run targeted test by name
cargo nextest run -p prb-core --test-threads=1 -E 'test(test_name)'

# Run all workspace tests
cargo nextest run --workspace

# Fallback if nextest is not installed
cargo test -p prb-core
cargo test --workspace
```

## Lint gate

```bash
cargo clippy -p prb-core -- -D warnings
cargo clippy --workspace -- -D warnings
```

## Key conventions

- **Error handling:** `thiserror` in library crates, `anyhow` in `prb-cli`
- **Shared deps:** declared in `workspace.dependencies` in root `Cargo.toml` with pinned versions
- **All crates:** edition = "2024" in their individual `Cargo.toml`
- **No `unsafe`** unless required by FFI (none expected in Phase 1)
- Never run `cargo` from inside a subdirectory — always run from the workspace root

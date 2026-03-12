# Contributing to PRB

Thank you for your interest in contributing to PRB. This document covers development setup, coding standards, and the pull request process.

## Development Environment

### Prerequisites

- Rust toolchain (2024 edition, 1.85+) via [rustup](https://rustup.rs/)
- libpcap development headers (see [Getting Started](docs/getting-started.md) for platform-specific instructions)
- Git

### Setup

```bash
git clone https://github.com/yourusername/prb.git
cd prb
cargo build
cargo test
```

### Pre-commit Hooks

Install Git pre-commit hooks to catch issues before they reach CI:

```bash
# Standard hooks (format, lint, fast tests)
bash scripts/install-hooks.sh

# Or use just:
just install-hooks
```

The hooks will automatically run on every `git commit` and block commits if checks fail.

To bypass hooks (not recommended):
```bash
git commit --no-verify
```

For faster hooks (skip tests):
```bash
bash scripts/install-hooks-fast.sh
```

### Useful Commands

```bash
# Build everything
cargo build

# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p prb-core

# Run clippy (must pass with no warnings)
cargo clippy --all-targets -- -D warnings

# Run a specific binary
cargo run -- ingest fixtures/grpc_sample.json

# Build release binary
cargo build --release

# Run benchmarks
cargo bench -p prb-pcap
cargo bench -p prb-detect
```

## Project Structure

PRB is a Cargo workspace with 19 crates under `crates/`:

| Layer | Crates |
|-------|--------|
| CLI | `prb-cli` |
| UI & Output | `prb-tui`, `prb-export`, `prb-query` |
| Core | `prb-core`, `prb-storage` |
| Ingestion | `prb-fixture`, `prb-pcap`, `prb-capture` |
| Protocols | `prb-detect`, `prb-grpc`, `prb-zmq`, `prb-dds` |
| Schemas | `prb-schema`, `prb-decode` |
| Plugins | `prb-plugin-api`, `prb-plugin-native`, `prb-plugin-wasm` |
| Experimental | `prb-ai` |

See [docs/architecture.md](docs/architecture.md) for the full design document.

## Coding Standards

### Rust Edition

PRB uses Rust 2024 edition. Use edition-appropriate syntax and idioms.

### Error Handling

- Use `CoreError` (from `prb-core`) for errors that cross crate boundaries
- Use `thiserror` for defining error enums
- Propagate errors with `?`; avoid `.unwrap()` in library code
- Include non-fatal warnings in `DebugEvent::warnings` rather than failing silently

### Testing

Every change should include tests:

- **Unit tests** -- `#[cfg(test)] mod tests` within source files
- **Integration tests** -- `tests/` directory in the crate
- **Snapshot tests** -- `insta` for TUI rendering and complex output
- **Property tests** -- `proptest` where applicable (parsers, encoders)
- **CLI tests** -- `assert_cmd` for end-to-end command tests

Run the full test suite before submitting:

```bash
cargo test
```

### Linting

Clippy must pass with no warnings:

```bash
cargo clippy --all-targets -- -D warnings
```

### Formatting

Use `rustfmt` with default settings:

```bash
cargo fmt
```

## Pull Request Process

1. **Fork and branch** -- Create a branch from `main` with a descriptive name (e.g., `feat/mqtt-decoder`, `fix/tcp-reassembly-ooo`)

2. **Make your changes** -- Follow the coding standards above. Keep commits focused and atomic.

3. **Test thoroughly** -- Run `cargo test` and `cargo clippy`. Add tests for new functionality.

4. **Write a clear PR description** -- Explain what the change does, why it's needed, and how to test it.

5. **One concern per PR** -- Keep PRs focused. Large changes should be broken into a series of smaller PRs.

### Commit Messages

Use conventional commit style:

```
feat(grpc): add gRPC-Web binary frame decoding
fix(pcap): handle out-of-order TCP segments in reassembly
test(query): add property tests for parser edge cases
docs: add TLS decryption guide for Java
refactor(detect): extract heuristic scoring into separate module
```

### What Makes a Good PR

- Focused on a single concern
- Includes tests
- Passes CI (`cargo test`, `cargo clippy`, `cargo fmt --check`)
- Has a clear description of the change and its motivation
- Does not break existing functionality

## Adding a New Protocol Decoder

To add a built-in protocol decoder:

1. Create a new crate: `crates/prb-<protocol>/`
2. Implement `ProtocolDecoder` from `prb-core`
3. Add a `DecoderFactory` to `prb-detect`
4. Add detection logic (port mapping, magic bytes, heuristics) to the detection engine
5. Wire it into `prb-pcap`'s pipeline
6. Add CLI support in `prb-cli` (new `--protocol` value)
7. Add tests with fixture PCAPs
8. Document in `docs/protocols.md`

For external decoders, use the plugin system instead. See [docs/plugin-development.md](docs/plugin-development.md).

## Adding a New Export Format

1. Add a new module in `crates/prb-export/src/`
2. Implement the `Exporter` trait
3. Register in `create_exporter()` and `supported_formats()`
4. Add the format variant to `ExportFormat` in `prb-cli`
5. Add tests
6. Document in `docs/export-formats.md`

## License

By contributing to PRB, you agree that your contributions will be licensed under the [GNU Affero General Public License v3.0](LICENSE).

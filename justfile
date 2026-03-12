# List available commands
default:
    @just --list

# Run all checks (format, lint, test) - use before committing
check: fmt-check clippy test

# Run full CI locally (format, lint, test, docs)
ci: fmt-check clippy test docs

# Format all code
fmt:
    cargo fmt --all

# Check formatting without modifying
fmt-check:
    cargo fmt --all -- --check

# Run clippy on all targets with strict lints
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Fix clippy issues automatically where possible
clippy-fix:
    cargo clippy --workspace --all-targets --fix --allow-dirty

# Run all tests
test:
    cargo test --workspace

# Run tests for a specific crate
test-crate CRATE:
    cargo test -p {{CRATE}}

# Run tests with nextest (faster)
nextest:
    cargo nextest run --workspace

# Generate and open coverage report (HTML)
coverage:
    cargo llvm-cov --workspace --html
    @echo "Opening coverage report..."
    @open target/llvm-cov/html/index.html || xdg-open target/llvm-cov/html/index.html || echo "Open target/llvm-cov/html/index.html manually"

# Generate coverage report (LCOV format)
coverage-lcov:
    cargo llvm-cov --workspace --lcov --output-path lcov.info

# Check coverage threshold (80%)
coverage-check:
    #!/usr/bin/env bash
    set -euo pipefail
    coverage=$(cargo llvm-cov --workspace --summary-only | grep -oP 'TOTAL.*\K[0-9.]+(?=%)')
    echo "Total coverage: $coverage%"
    if (( $(echo "$coverage < 80.0" | bc -l) )); then
        echo "[FAIL] Coverage $coverage% is below 80% threshold"
        exit 1
    fi
    echo "[PASS] Coverage $coverage% meets threshold"

# Run benchmarks
bench:
    cargo bench --workspace

# Run benchmarks for a specific crate
bench-crate CRATE:
    cargo bench -p {{CRATE}}

# Build release binary
build:
    cargo build --release -p prb-cli

# Build workspace in release mode
build-all:
    cargo build --workspace --release

# Clean all build artifacts
clean:
    cargo clean

# Security audit
audit:
    cargo audit

# Check dependencies for updates
outdated:
    cargo outdated --root-deps-only

# Check for duplicate dependencies
duplicates:
    cargo tree --duplicates

# Run cargo-deny checks (licenses, bans, advisories)
deny:
    cargo deny check

# Generate and open documentation
docs:
    cargo doc --workspace --no-deps --open

# Check documentation builds without warnings
docs-check:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# Install development dependencies
setup:
    @echo "Installing developer tools..."
    cargo install cargo-llvm-cov --locked
    cargo install cargo-audit --locked
    cargo install cargo-deny --locked
    cargo install cargo-outdated --locked
    cargo install cargo-nextest --locked
    cargo install just --locked
    @echo "[PASS] Developer tools installed!"

# Install pre-commit hooks
install-hooks:
    @echo "Installing pre-commit hooks..."
    @bash scripts/install-hooks.sh
    @echo "[PASS] Pre-commit hooks installed!"

# Run TUI with test data
tui:
    cargo run -p prb-cli -- tui fixtures/sample.pcap

# Run CLI commands for testing
ingest FILE:
    cargo run -p prb-cli -- ingest {{FILE}}

# Watch for changes and run tests
watch:
    cargo watch -x 'test --workspace'

# Run pre-commit checks (fast version of check)
pre-commit: fmt-check clippy test-quick

# Run quick tests (lib and bins only, skip slow integration tests)
test-quick:
    cargo test --workspace --lib --bins

# Fix all auto-fixable issues (format + clippy)
fix: fmt clippy-fix

# Full development cycle: fix issues, run tests, check coverage
dev: fix test coverage-check

# Profile release build
profile:
    cargo build --release -p prb-cli
    @echo "Release binary at: target/release/prb"
    @echo "Run with: hyperfine 'target/release/prb ingest <file>'"

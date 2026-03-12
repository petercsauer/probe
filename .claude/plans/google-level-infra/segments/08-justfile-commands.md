---
segment: 08
title: Justfile & Commands
depends_on: []
risk: 1
complexity: Low
cycle_budget: 2
estimated_lines: 1 new file (~150 lines)
---

# Segment 08: Justfile & Developer Commands

## Context

Create a `justfile` with convenient developer commands to streamline common workflows. `just` is a modern command runner that provides simple, memorable commands for complex operations.

## Current State

- No command runner
- Developers must remember long cargo commands
- No single "check everything" command

## Goal

Provide convenient `just` commands for all common developer operations.

## Exit Criteria

1. [ ] `justfile` created in workspace root
2. [ ] Commands for format, lint, test, coverage, build, docs
3. [ ] `just check` runs all quality gates
4. [ ] `just ci` simulates full CI locally
5. [ ] `just setup` installs developer dependencies
6. [ ] Commands documented with descriptions
7. [ ] Manual test: Run each command and verify it works
8. [ ] README updated with just usage instructions

## Implementation Plan

### Create Justfile

Create `/Users/psauer/probe/justfile`:

```justfile
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
        echo "❌ Coverage $coverage% is below 80% threshold"
        exit 1
    fi
    echo "✅ Coverage $coverage% meets threshold"

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
    @echo "✅ Developer tools installed!"

# Install pre-commit hooks
install-hooks:
    @echo "Installing pre-commit hooks..."
    @bash scripts/install-hooks.sh
    @echo "✅ Pre-commit hooks installed!"

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
```

### Update README

Add to `/Users/psauer/probe/README.md`:

```markdown
## Development

This project uses [`just`](https://github.com/casey/just) for task automation.

### Setup

```bash
# Install just
cargo install just

# Install development dependencies
just setup

# Install pre-commit hooks (optional but recommended)
just install-hooks
```

### Common Commands

```bash
# See all available commands
just

# Run all checks before committing
just check

# Run full CI locally
just ci

# Generate coverage report
just coverage

# Run tests
just test

# Build release binary
just build
```

See `justfile` for all available commands.
```

## Files to Create/Modify

1. `/Users/psauer/probe/justfile` (new, ~150 lines)
2. `/Users/psauer/probe/README.md` (add developer section)

## Test Plan

1. Create justfile
2. Install just if not present:
   ```bash
   cargo install just
   ```
3. Test each command:
   ```bash
   just                  # List commands
   just check           # Should run fmt-check, clippy, test
   just fmt             # Should format code
   just coverage        # Should generate report
   just test           # Should run tests
   just build          # Should build release binary
   just setup          # Should install tools
   ```
4. Verify commands work on clean checkout
5. Update README with just instructions
6. Commit: "infra: Add justfile for developer commands"

## Blocked By

None - justfile is standalone tooling.

## Blocks

None - improves developer experience but doesn't block other work.

## Success Metrics

- Justfile created with all commands
- All commands functional
- `just check` runs successfully
- `just ci` simulates CI
- `just setup` installs tools
- README documents usage

## Notes

- just is similar to make but simpler and more ergonomic
- Commands have built-in documentation (visible with `just --list`)
- Can be extended over time with new commands
- Particularly useful for onboarding new developers
- Cross-platform (works on Linux, macOS, Windows)

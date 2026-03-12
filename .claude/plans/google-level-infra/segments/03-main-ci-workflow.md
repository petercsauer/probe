---
segment: 03
title: Main CI Workflow
depends_on: [1, 2]
risk: 3
complexity: Medium
cycle_budget: 5
estimated_lines: 1 new file (~250 lines)
---

# Segment 03: Main CI Workflow

## Context

Create the primary CI workflow that runs on every push and pull request. This workflow will enforce all quality gates, run tests on multiple platforms, generate coverage reports, and ensure code quality.

## Current State

No CI/CD infrastructure exists:
- `.github/workflows/` directory doesn't exist
- No automated testing on push/PR
- No coverage tracking
- No multi-platform verification

## Goal

Establish comprehensive CI pipeline with format checking, linting, testing, coverage, security scanning, and documentation validation across Linux, macOS, and Windows.

## Exit Criteria

1. [ ] `.github/workflows/ci.yml` created with all jobs
2. [ ] CI runs on push to main and all PRs
3. [ ] Format checking job passes
4. [ ] Clippy linting job passes
5. [ ] Tests run on Linux, macOS, Windows
6. [ ] Coverage report generated and uploaded to Codecov
7. [ ] Coverage threshold check enforces 80% minimum
8. [ ] Security audit job runs
9. [ ] Documentation build job validates rustdoc
10. [ ] Benchmark job runs on main branch
11. [ ] Manual test: Create a test PR and verify all jobs run

## Implementation Plan

### Create CI Workflow File

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  # Fast feedback - formatting and basic checks
  check:
    name: Format & Lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - name: Check formatting
        run: cargo fmt --all -- --check
      - name: Clippy
        run: cargo clippy --workspace --all-targets -- -D warnings

  # Security scanning
  security:
    name: Security Audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Install cargo-audit
        run: cargo install cargo-audit --locked
      - name: Run audit
        run: cargo audit
      - name: Install cargo-deny
        run: cargo install cargo-deny --locked
      - name: Run deny
        run: cargo deny check

  # Multi-platform testing
  test:
    name: Test (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.os }}

      # Install libpcap (platform-specific)
      - name: Install libpcap (Ubuntu)
        if: matrix.os == 'ubuntu-latest'
        run: sudo apt-get update && sudo apt-get install -y libpcap-dev

      - name: Install libpcap (macOS)
        if: matrix.os == 'macos-latest'
        run: brew install libpcap

      - name: Install WinPcap (Windows)
        if: matrix.os == 'windows-latest'
        run: choco install winpcap
        continue-on-error: true

      - name: Build
        run: cargo build --workspace --all-targets

      - name: Test
        run: cargo test --workspace

  # Coverage reporting (Linux only)
  coverage:
    name: Code Coverage
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: llvm-tools-preview
      - uses: Swatinem/rust-cache@v2

      - name: Install libpcap
        run: sudo apt-get update && sudo apt-get install -y libpcap-dev

      - name: Install cargo-llvm-cov
        run: cargo install cargo-llvm-cov --locked

      - name: Generate coverage
        run: cargo llvm-cov --workspace --lcov --output-path lcov.info

      - name: Upload to Codecov
        uses: codecov/codecov-action@v4
        with:
          files: lcov.info
          fail_ci_if_error: true
          token: ${{ secrets.CODECOV_TOKEN }}

      - name: Coverage threshold check
        run: |
          coverage=$(cargo llvm-cov --workspace --summary-only | grep -oP 'TOTAL.*\K[0-9.]+(?=%)')
          echo "Total coverage: $coverage%"
          if (( $(echo "$coverage < 80.0" | bc -l) )); then
            echo "❌ Coverage $coverage% is below 80% threshold"
            exit 1
          fi
          echo "✅ Coverage $coverage% meets threshold"

  # Documentation build
  docs:
    name: Documentation
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      - name: Check docs
        run: cargo doc --workspace --no-deps --document-private-items
        env:
          RUSTDOCFLAGS: "-D warnings"

  # Benchmarks (main branch only, don't block PRs)
  benchmark:
    name: Benchmarks
    runs-on: ubuntu-latest
    if: github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      - name: Install libpcap
        run: sudo apt-get update && sudo apt-get install -y libpcap-dev

      - name: Run benchmarks
        run: cargo bench --workspace -- --output-format bencher | tee benchmark-output.txt
        continue-on-error: true

      - name: Store benchmark result
        uses: benchmark-action/github-action-benchmark@v1
        if: success()
        with:
          tool: 'cargo'
          output-file-path: benchmark-output.txt
          github-token: ${{ secrets.GITHUB_TOKEN }}
          auto-push: true
```

### Required Setup

1. **Create .github/workflows directory**:
   ```bash
   mkdir -p .github/workflows
   ```

2. **Codecov Token** (for coverage upload):
   - Sign up at https://codecov.io
   - Add repository
   - Copy token
   - Add as GitHub secret: `CODECOV_TOKEN`

3. **Optional: Setup benchmark storage**:
   - Benchmark job stores results in gh-pages branch
   - May need to enable GitHub Pages in repo settings

## Files to Create

- `.github/workflows/ci.yml` (~250 lines)

## Test Plan

1. Create `.github/workflows/ci.yml`
2. Create `.github` directory if needed
3. Commit and push to a test branch
4. Open a PR to main
5. Verify all jobs run:
   - ✅ Format & Lint
   - ✅ Security Audit
   - ✅ Test (ubuntu-latest)
   - ✅ Test (macos-latest)
   - ✅ Test (windows-latest)
   - ✅ Code Coverage
   - ✅ Documentation
6. Verify coverage report uploads to Codecov
7. Verify coverage threshold check works
8. Merge to main and verify benchmark job runs
9. Check GitHub Actions tab for workflow runs

## Blocked By

- Segment 01 (Fix Existing Issues) - tests must pass
- Segment 02 (Quality Configs) - configs must exist

## Blocks

- Segment 04 (Release Automation) - uses same CI patterns
- Segment 06 (Coverage Analysis) - needs coverage CI job

## Success Metrics

- CI workflow file created
- All jobs defined and running
- Multi-platform tests passing
- Coverage report generated
- Threshold enforcement working
- Documentation building cleanly

## Notes

- Windows tests may be flaky due to WinPcap installation
- Consider making Windows tests optional initially
- Codecov token required for coverage uploads
- Benchmark job only runs on main to avoid noise
- Failed benchmarks don't block CI (continue-on-error)

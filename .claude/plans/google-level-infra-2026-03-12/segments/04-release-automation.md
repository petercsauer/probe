---
segment: 04
title: Release Automation
depends_on: [3]
risk: 4
complexity: Medium
cycle_budget: 4
estimated_lines: 2 new files (~150 lines total)
---

# Segment 04: Release Automation

## Context

Automate the release process with multi-platform binary builds triggered by Git tags. When a version tag is pushed (e.g., `v0.2.0`), automatically build binaries for Linux, macOS (Intel + ARM), and Windows, then create a GitHub Release with all artifacts.

## Current State

- No release automation
- Manual build and distribution process
- No multi-platform binary artifacts

## Goal

Implement automated release workflow that builds, packages, and publishes multi-platform binaries on tag push.

## Exit Criteria

1. [ ] `.github/workflows/release.yml` created
2. [ ] Workflow triggers on `v*.*.*` tags
3. [ ] Builds for Linux x86_64, macOS x86_64, macOS ARM64, Windows x86_64
4. [ ] Binaries packaged as `.tar.gz` (Unix) and `.zip` (Windows)
5. [ ] GitHub Release created automatically with all artifacts
6. [ ] Release notes auto-generated from commits
7. [ ] Manual test: Push a test tag, verify release creation
8. [ ] Manual test: Download and run released binary

## Implementation Plan

### File 1: Release Workflow

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*.*.*'

env:
  CARGO_TERM_COLOR: always

jobs:
  create-release:
    name: Create Release
    runs-on: ubuntu-latest
    outputs:
      upload_url: ${{ steps.create_release.outputs.upload_url }}
      version: ${{ steps.get_version.outputs.version }}
    steps:
      - name: Get version from tag
        id: get_version
        run: echo "version=${GITHUB_REF#refs/tags/}" >> $GITHUB_OUTPUT

      - name: Create Release
        id: create_release
        uses: softprops/action-gh-release@v1
        with:
          name: Release ${{ steps.get_version.outputs.version }}
          draft: false
          prerelease: false
          generate_release_notes: true

  build:
    name: Build (${{ matrix.target }})
    needs: create-release
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        include:
          # Linux
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact_name: prb
            asset_name: prb-linux-x86_64.tar.gz

          # macOS Intel
          - os: macos-latest
            target: x86_64-apple-darwin
            artifact_name: prb
            asset_name: prb-macos-x86_64.tar.gz

          # macOS ARM (M1/M2)
          - os: macos-latest
            target: aarch64-apple-darwin
            artifact_name: prb
            asset_name: prb-macos-arm64.tar.gz

          # Windows
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact_name: prb.exe
            asset_name: prb-windows-x86_64.zip

    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.target }}

      # Install platform-specific dependencies
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

      - name: Build release binary
        run: cargo build --release --target ${{ matrix.target }} -p prb-cli

      # Package Unix binaries (.tar.gz)
      - name: Package (Unix)
        if: matrix.os != 'windows-latest'
        run: |
          cd target/${{ matrix.target }}/release
          tar czf ${{ matrix.asset_name }} ${{ matrix.artifact_name }}
          mv ${{ matrix.asset_name }} ../../../

      # Package Windows binary (.zip)
      - name: Package (Windows)
        if: matrix.os == 'windows-latest'
        run: |
          cd target/${{ matrix.target }}/release
          7z a ${{ matrix.asset_name }} ${{ matrix.artifact_name }}
          mv ${{ matrix.asset_name }} ../../../

      - name: Upload Release Asset
        uses: softprops/action-gh-release@v1
        with:
          files: ${{ matrix.asset_name }}
```

### File 2: Dependabot Configuration

Create `.github/dependabot.yml` for automated dependency updates:

```yaml
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
      day: "monday"
      time: "09:00"
      timezone: "America/New_York"
    open-pull-requests-limit: 10
    reviewers:
      - "your-team-or-username"
    labels:
      - "dependencies"
      - "rust"
    commit-message:
      prefix: "deps"
      include: "scope"

  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
    open-pull-requests-limit: 5
    labels:
      - "dependencies"
      - "ci"
    commit-message:
      prefix: "ci"
```

## Files to Create

1. `.github/workflows/release.yml` (~120 lines)
2. `.github/dependabot.yml` (~30 lines)

## Test Plan

1. Create release workflow file
2. Create dependabot config
3. Test workflow with a version tag:
   ```bash
   git tag v0.1.1-test
   git push origin v0.1.1-test
   ```
4. Monitor GitHub Actions for workflow execution
5. Verify all platform builds complete
6. Verify GitHub Release is created with:
   - Release notes (auto-generated from commits)
   - 4 binary artifacts (Linux, macOS x2, Windows)
7. Download and test one of the binaries:
   ```bash
   # Example for Linux
   wget https://github.com/yourusername/prb/releases/download/v0.1.1-test/prb-linux-x86_64.tar.gz
   tar xzf prb-linux-x86_64.tar.gz
   ./prb --version
   ```
8. Delete test release and tag after verification
9. Commit: "infra: Add release automation workflow"

## Blocked By

- Segment 03 (Main CI Workflow) - uses similar job patterns

## Blocks

None - release automation is independent.

## Success Metrics

- Release workflow created
- Dependabot configured
- Test release completes successfully
- All platform binaries build correctly
- Artifacts uploadable and functional
- Release notes auto-generated

## Notes

- Windows build may need WinPcap pre-installed in runner
- macOS ARM cross-compilation works on macOS Intel runners
- Consider adding checksums (SHA256) for binary verification
- Update repository README with installation instructions
- Set up branch protection to prevent accidental tag pushes

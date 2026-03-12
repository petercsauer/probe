---
segment: 02
title: Quality Configuration Files
depends_on: []
risk: 1
complexity: Low
cycle_budget: 2
estimated_lines: 4 new files
---

# Segment 02: Quality Configuration Files

## Context

Establish strict quality standards with configuration files that will be enforced in CI. These configs define formatting rules, linting strictness, and security policies.

## Current State

No quality configuration files exist:
- No `rustfmt.toml` - using default formatting
- No `clippy.toml` - using default lints
- No `.cargo/config.toml` - no workspace-level rustflags
- No `deny.toml` - no security/license scanning

## Goal

Create comprehensive quality configuration files that enforce Google-level standards.

## Exit Criteria

1. [ ] `rustfmt.toml` created in workspace root with strict formatting rules
2. [ ] `clippy.toml` created in workspace root with strict linting thresholds
3. [ ] `.cargo/config.toml` created with strict rustflags (warnings as errors)
4. [ ] `deny.toml` created with security/license/source policies
5. [ ] Manual test: `cargo fmt --check` passes with new config
6. [ ] Manual test: `cargo clippy --workspace --all-targets` respects new config
7. [ ] Manual test: `cargo deny check` passes

## Implementation Plan

### File 1: rustfmt.toml

Create `/Users/psauer/probe/rustfmt.toml`:

```toml
# Strict formatting for production code
edition = "2024"
max_width = 100
hard_tabs = false
tab_spaces = 4
newline_style = "Unix"
use_small_heuristics = "Default"

# Import organization
reorder_imports = true
reorder_modules = true
imports_granularity = "Crate"

# Code cleanup
remove_nested_parens = true
use_try_shorthand = true

# Documentation
format_code_in_doc_comments = true
normalize_comments = true
wrap_comments = true
comment_width = 80

# String formatting
format_strings = true
normalize_doc_attributes = true

# Trailing elements
trailing_comma = "Vertical"
trailing_semicolon = true
```

### File 2: clippy.toml

Create `/Users/psauer/probe/clippy.toml`:

```toml
# Clippy configuration for strict linting
cognitive-complexity-threshold = 30
single-char-binding-names-threshold = 4
too-many-arguments-threshold = 7
type-complexity-threshold = 250
too-many-lines-threshold = 300
```

### File 3: .cargo/config.toml

Create `/Users/psauer/probe/.cargo/config.toml`:

```toml
[build]
# Ensure fast builds in dev, optimized in release
incremental = true

[target.'cfg(all())']
# Strict warnings - all warnings are errors
rustflags = [
    "-D", "warnings",
    "-D", "clippy::all",
    "-D", "clippy::pedantic",
    "-D", "clippy::cargo",
    "-W", "clippy::nursery",  # Warn on nursery, don't error
]

# Platform-specific: Unix-like systems
[target.'cfg(unix)']
rustflags = [
    "-D", "warnings",
    "-D", "clippy::all",
    "-D", "clippy::pedantic",
    "-D", "clippy::cargo",
    "-W", "clippy::nursery",
]

# Platform-specific: Windows
[target.'cfg(windows)']
rustflags = [
    "-D", "warnings",
    "-D", "clippy::all",
    "-D", "clippy::pedantic",
    "-D", "clippy::cargo",
    "-W", "clippy::nursery",
]

[alias]
# Convenient aliases
xtask = "run --package xtask --"
cov = "llvm-cov --workspace --lcov --output-path lcov.info"
```

### File 4: deny.toml

Create `/Users/psauer/probe/deny.toml`:

```toml
# cargo-deny configuration
# Security and supply chain validation

[advisories]
version = 2
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/rustsec/advisory-db"]
# Ignore specific advisories (add as needed)
ignore = []
# Warn on all severities
severity-threshold = "low"

[licenses]
version = 2
# Allow common permissive licenses
allow = [
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "MIT",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Unicode-DFS-2016",
    "Unlicense",
    "0BSD",
]
# Deny copyleft licenses (project is AGPL-3.0 so this is policy)
deny = [
    "GPL-2.0",
    "GPL-3.0",
    "LGPL-2.0",
    "LGPL-2.1",
    "LGPL-3.0",
]
copyleft = "deny"
allow-osi-fsf-free = "both"
private = { ignore = true }
confidence-threshold = 0.8

[bans]
# Warn on multiple versions of the same crate
multiple-versions = "warn"
# Deny wildcard dependencies
wildcards = "deny"
highlight = "all"
workspace-default-features = "allow"
# Allow certain duplicates if necessary
allow-duplicate = []
# Deny specific crates if needed
deny = []
# Skip checking for duplicates in dev/build dependencies
skip = []
skip-tree = []

[sources]
# Only allow crates.io (deny git dependencies in production)
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
# Allow git dependencies in dev only
[sources.allow-org]
github = []
```

## Files to Create

1. `/Users/psauer/probe/rustfmt.toml` (~35 lines)
2. `/Users/psauer/probe/clippy.toml` (~7 lines)
3. `/Users/psauer/probe/.cargo/config.toml` (~40 lines)
4. `/Users/psauer/probe/deny.toml` (~65 lines)

## Test Plan

1. Create all four configuration files
2. Run `cargo fmt --all` - should format according to new rules
3. Run `cargo fmt --all -- --check` - should pass
4. Run `cargo clippy --workspace --all-targets` - should use new thresholds
5. Install cargo-deny: `cargo install cargo-deny`
6. Run `cargo deny check` - should pass (or identify issues to fix)
7. Verify: Check that clippy respects cognitive complexity threshold
8. Commit: "infra: Add strict quality configuration files"

## Blocked By

None - these are standalone config files.

## Blocks

Segment 03 (Main CI Workflow) - CI will reference these configs.

## Success Metrics

- All 4 config files created
- `cargo fmt --check` passes
- `cargo clippy` uses strict linting
- `cargo deny check` passes
- Configs committed to repo

## Notes

- These configs are additive - they don't change existing code
- CI will enforce these standards in later segments
- deny.toml may reveal dependency issues that need addressing
- Consider relaxing some clippy lints if they're too strict for the team

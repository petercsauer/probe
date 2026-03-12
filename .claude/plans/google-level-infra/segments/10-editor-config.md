---
segment: 10
title: Editor Configuration
depends_on: []
risk: 1
complexity: Low
cycle_budget: 1
estimated_lines: 2 new files
---

# Segment 10: Editor Configuration

## Context

Provide standardized editor configuration files so all developers have consistent formatting, linting, and IDE behavior regardless of their editor choice.

## Current State

- No editor configuration files
- Inconsistent editor settings across team
- Developers must manually configure their IDEs

## Goal

Create .editorconfig and VS Code workspace settings for consistent developer experience.

## Exit Criteria

1. [ ] `.editorconfig` created for universal editor support
2. [ ] `.vscode/settings.json` created for VS Code users
3. [ ] `.vscode/extensions.json` created with recommended extensions
4. [ ] Settings enforce format-on-save, clippy on check
5. [ ] Manual test: Open project in VS Code, verify settings apply
6. [ ] Manual test: Edit a Rust file, verify auto-format works

## Implementation Plan

### File 1: EditorConfig

Create `/Users/psauer/probe/.editorconfig`:

```ini
# EditorConfig for PRB project
# Universal editor configuration
# See: https://editorconfig.org

root = true

# Default for all files
[*]
charset = utf-8
end_of_line = lf
insert_final_newline = true
trim_trailing_whitespace = true

# Rust files
[*.rs]
indent_style = space
indent_size = 4
max_line_length = 100

# TOML files
[*.toml]
indent_style = space
indent_size = 2

# YAML files (workflows)
[*.{yml,yaml}]
indent_style = space
indent_size = 2

# JSON files
[*.json]
indent_style = space
indent_size = 2

# Markdown files
[*.md]
trim_trailing_whitespace = false
max_line_length = 80

# Makefile and Justfile
[{Makefile,justfile}]
indent_style = tab
indent_size = 4

# Shell scripts
[*.sh]
indent_style = space
indent_size = 2

# Cargo.lock (don't format)
[Cargo.lock]
insert_final_newline = false
```

### File 2: VS Code Settings

Create `/Users/psauer/probe/.vscode/settings.json`:

```json
{
  // Rust analyzer configuration
  "rust-analyzer.check.command": "clippy",
  "rust-analyzer.check.extraArgs": [
    "--all-targets",
    "--",
    "-D",
    "warnings"
  ],
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.cargo.allTargets": true,
  "rust-analyzer.procMacro.enable": true,
  "rust-analyzer.inlayHints.enable": true,

  // Formatting
  "editor.formatOnSave": true,
  "editor.formatOnPaste": false,
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer",
    "editor.rulers": [100],
    "editor.tabSize": 4,
    "editor.insertSpaces": true
  },

  // TOML formatting
  "[toml]": {
    "editor.defaultFormatter": "tamasfe.even-better-toml",
    "editor.tabSize": 2
  },

  // Markdown
  "[markdown]": {
    "editor.wordWrap": "on",
    "editor.rulers": [80]
  },

  // JSON
  "[json]": {
    "editor.defaultFormatter": "vscode.json-language-features",
    "editor.tabSize": 2
  },

  // YAML
  "[yaml]": {
    "editor.defaultFormatter": "redhat.vscode-yaml",
    "editor.tabSize": 2
  },

  // Files
  "files.eol": "\n",
  "files.insertFinalNewline": true,
  "files.trimTrailingWhitespace": true,
  "files.exclude": {
    "**/target": true,
    "**/.git": true
  },

  // Search
  "search.exclude": {
    "**/target": true,
    "**/Cargo.lock": true
  },

  // Terminal
  "terminal.integrated.env.linux": {
    "RUST_BACKTRACE": "1"
  },
  "terminal.integrated.env.osx": {
    "RUST_BACKTRACE": "1"
  },

  // Code actions
  "editor.codeActionsOnSave": {
    "source.fixAll": "explicit",
    "source.organizeImports": "explicit"
  },

  // Clippy warnings
  "rust-analyzer.diagnostics.disabled": [],
  "rust-analyzer.diagnostics.enable": true,
  "rust-analyzer.diagnostics.warningsAsHint": [],
  "rust-analyzer.diagnostics.warningsAsInfo": []
}
```

### File 3: Recommended Extensions

Create `/Users/psauer/probe/.vscode/extensions.json`:

```json
{
  "recommendations": [
    // Core Rust
    "rust-lang.rust-analyzer",

    // TOML
    "tamasfe.even-better-toml",

    // Testing
    "hbenl.vscode-test-explorer",
    "swellaby.vscode-rust-test-adapter",

    // Git
    "eamodio.gitlens",

    // Markdown
    "yzhang.markdown-all-in-one",
    "davidanson.vscode-markdownlint",

    // YAML (for CI workflows)
    "redhat.vscode-yaml",

    // Code quality
    "usernamehw.errorlens",

    // Coverage
    "ryanluker.vscode-coverage-gutters"
  ]
}
```

## Files to Create

1. `/Users/psauer/probe/.editorconfig` (~60 lines)
2. `/Users/psauer/probe/.vscode/settings.json` (~85 lines)
3. `/Users/psauer/probe/.vscode/extensions.json` (~25 lines)

## Test Plan

1. Create all three configuration files
2. Close and reopen VS Code
3. Verify rust-analyzer loads and shows clippy warnings
4. Edit a Rust file and save - verify auto-format works
5. Create a line longer than 100 chars - verify ruler at column 100
6. Verify recommended extensions prompt appears
7. Test in other editors that support .editorconfig (IntelliJ, Vim, etc.)
8. Commit: "infra: Add editor configuration for consistent development"

## Blocked By

None - editor config is standalone.

## Blocks

None - improves developer experience.

## Success Metrics

- All config files created
- VS Code applies settings automatically
- Format-on-save works
- Clippy runs on check
- EditorConfig works in multiple editors
- Recommended extensions listed

## Notes

- .editorconfig is universal (works with most editors)
- .vscode/ is VS Code specific but most Rust devs use VS Code
- Can add .idea/ for IntelliJ users if needed
- Settings don't override user's global preferences aggressively
- Some settings require rust-analyzer extension installed

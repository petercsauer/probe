---
segment: 09
title: Pre-commit Hooks
depends_on: [2]
risk: 2
complexity: Low
cycle_budget: 2
estimated_lines: 2 new files
---

# Segment 09: Pre-commit Hooks

## Context

Install Git pre-commit hooks that enforce quality gates locally before code reaches CI. This provides fast feedback and prevents broken commits from entering the repository.

## Current State

- No pre-commit hooks
- Quality issues can reach the repository
- Developers learn about issues only in CI (slow feedback)

## Goal

Implement pre-commit hooks that run format checks, clippy, and fast tests before allowing commits.

## Exit Criteria

1. [ ] `scripts/install-hooks.sh` script created
2. [ ] Pre-commit hook installed in `.git/hooks/pre-commit`
3. [ ] Hook runs format check, clippy, and fast tests
4. [ ] Hook blocks commits when checks fail
5. [ ] Hook provides clear error messages
6. [ ] Hook can be bypassed with `--no-verify` if needed
7. [ ] Installation documented in README
8. [ ] Manual test: Make a formatting error, attempt commit, verify blocked

## Implementation Plan

### File 1: Hook Installation Script

Create `/Users/psauer/probe/scripts/install-hooks.sh`:

```bash
#!/bin/bash
# Install Git pre-commit hooks for quality enforcement

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
HOOK_DIR="$REPO_ROOT/.git/hooks"
HOOK_FILE="$HOOK_DIR/pre-commit"

echo "Installing pre-commit hooks..."

# Ensure hooks directory exists
mkdir -p "$HOOK_DIR"

# Create pre-commit hook
cat > "$HOOK_FILE" <<'EOF'
#!/bin/bash
# Pre-commit hook for PRB project
# Enforces formatting, linting, and fast tests before commit

set -e

echo "🔍 Running pre-commit checks..."
echo ""

# 1. Format check
echo "→ Checking formatting..."
if ! cargo fmt --all -- --check; then
    echo ""
    echo "❌ Format check failed!"
    echo "   Run: cargo fmt --all"
    echo "   Or use: just fmt"
    echo ""
    exit 1
fi
echo "✅ Format check passed"
echo ""

# 2. Clippy
echo "→ Running clippy..."
if ! cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tee /tmp/clippy-output.txt; then
    echo ""
    echo "❌ Clippy check failed!"
    echo "   Fix warnings shown above"
    echo "   Or use: just clippy-fix"
    echo ""
    exit 1
fi
echo "✅ Clippy passed"
echo ""

# 3. Fast tests (lib + bins only, skip slow integration tests)
echo "→ Running fast tests (lib + bins)..."
if ! cargo test --workspace --lib --bins; then
    echo ""
    echo "❌ Tests failed!"
    echo "   Fix failing tests before committing"
    echo ""
    exit 1
fi
echo "✅ Tests passed"
echo ""

echo "✅ All pre-commit checks passed!"
echo ""
echo "💡 Tip: To skip hooks (not recommended), use: git commit --no-verify"
EOF

# Make hook executable
chmod +x "$HOOK_FILE"

echo "✅ Pre-commit hook installed at: $HOOK_FILE"
echo ""
echo "The hook will run on every commit and check:"
echo "  • Code formatting (cargo fmt)"
echo "  • Linting (cargo clippy)"
echo "  • Fast tests (lib + bins)"
echo ""
echo "To bypass the hook (not recommended): git commit --no-verify"
```

### File 2: Optional - Fast Hook Variant

Create `/Users/psauer/probe/scripts/install-hooks-fast.sh`:

```bash
#!/bin/bash
# Install lighter pre-commit hooks (format + clippy only, no tests)
# Use for faster commits when you're running tests separately

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
HOOK_DIR="$REPO_ROOT/.git/hooks"
HOOK_FILE="$HOOK_DIR/pre-commit"

echo "Installing fast pre-commit hooks (no tests)..."

mkdir -p "$HOOK_DIR"

cat > "$HOOK_FILE" <<'EOF'
#!/bin/bash
# Fast pre-commit hook (format + clippy only)

set -e

echo "🔍 Running fast pre-commit checks..."

# Format check
echo "→ Checking formatting..."
if ! cargo fmt --all -- --check; then
    echo "❌ Format check failed! Run: cargo fmt --all"
    exit 1
fi
echo "✅ Format OK"

# Clippy
echo "→ Running clippy..."
if ! cargo clippy --workspace --all-targets -- -D warnings 2>&1 | head -20; then
    echo "❌ Clippy failed! Fix warnings above"
    exit 1
fi
echo "✅ Clippy OK"

echo "✅ Fast checks passed!"
EOF

chmod +x "$HOOK_FILE"

echo "✅ Fast pre-commit hook installed"
echo "   (format + clippy only, skips tests for speed)"
```

### Update CONTRIBUTING.md

Add to `/Users/psauer/probe/CONTRIBUTING.md`:

```markdown
## Pre-commit Hooks

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
```

## Files to Create/Modify

1. `/Users/psauer/probe/scripts/install-hooks.sh` (new, ~80 lines)
2. `/Users/psauer/probe/scripts/install-hooks-fast.sh` (new, ~50 lines)
3. `/Users/psauer/probe/CONTRIBUTING.md` (add pre-commit section)

## Test Plan

1. Create hook installation scripts
2. Make scripts executable:
   ```bash
   chmod +x scripts/install-hooks.sh
   chmod +x scripts/install-hooks-fast.sh
   ```
3. Run installation:
   ```bash
   bash scripts/install-hooks.sh
   ```
4. Verify hook installed:
   ```bash
   cat .git/hooks/pre-commit
   ```
5. Test hook blocks bad commits:
   ```bash
   # Make a formatting error
   echo "fn bad_format(){}" >> crates/prb-core/src/lib.rs
   git add crates/prb-core/src/lib.rs
   git commit -m "test"
   # Should fail with format error
   ```
6. Fix and verify hook passes:
   ```bash
   git restore crates/prb-core/src/lib.rs
   cargo fmt --all
   git add .
   git commit -m "test"
   # Should pass
   ```
7. Test bypass works:
   ```bash
   git commit --no-verify -m "test"
   # Should commit without running hooks
   ```
8. Update CONTRIBUTING.md
9. Commit: "infra: Add pre-commit hooks for quality enforcement"

## Blocked By

- Segment 02 (Quality Configs) - hooks rely on clippy/rustfmt configs

## Blocks

None - improves developer experience.

## Success Metrics

- Hook installation scripts created
- Hooks functional and block bad commits
- Clear error messages when checks fail
- Documentation updated
- Manual tests pass

## Notes

- Hooks run locally, don't affect CI
- Can be bypassed with --no-verify if needed (emergency commits)
- Fast variant available for developers who prefer speed
- Consider making hooks warn-only initially for team adoption
- Hooks are not committed to repo (each developer installs them)
- Some teams prefer tools like pre-commit.com or husky for hook management

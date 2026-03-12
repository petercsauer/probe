#!/bin/bash
# Install Git pre-commit hooks for quality enforcement

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
# Use git-common-dir to support worktrees
GIT_DIR="$(cd "$REPO_ROOT" && git rev-parse --git-common-dir)"
HOOK_DIR="$GIT_DIR/hooks"
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

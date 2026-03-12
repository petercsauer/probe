#!/bin/bash
# Install lighter pre-commit hooks (format + clippy only, no tests)
# Use for faster commits when you're running tests separately

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
# Use git-common-dir to support worktrees
GIT_DIR="$(cd "$REPO_ROOT" && git rev-parse --git-common-dir)"
HOOK_DIR="$GIT_DIR/hooks"
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

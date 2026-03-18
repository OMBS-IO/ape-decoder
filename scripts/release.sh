#!/bin/bash
# Release script for ape-decoder
#
# Usage:
#   ./scripts/release.sh patch    # 0.1.0 -> 0.1.1
#   ./scripts/release.sh minor    # 0.1.0 -> 0.2.0
#   ./scripts/release.sh major    # 0.1.0 -> 1.0.0
#   ./scripts/release.sh 0.2.0    # explicit version

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_DIR"

# --- Parse argument ---
BUMP="${1:-}"
if [ -z "$BUMP" ]; then
    echo "Usage: ./scripts/release.sh <patch|minor|major|x.y.z>"
    exit 1
fi

# --- Get current version from Cargo.toml ---
CURRENT=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

# --- Compute new version ---
case "$BUMP" in
    patch) NEW_VERSION="$MAJOR.$MINOR.$((PATCH + 1))" ;;
    minor) NEW_VERSION="$MAJOR.$((MINOR + 1)).0" ;;
    major) NEW_VERSION="$((MAJOR + 1)).0.0" ;;
    *.*.*)  NEW_VERSION="$BUMP" ;;
    *)
        echo "ERROR: Invalid bump type '$BUMP'. Use patch, minor, major, or x.y.z"
        exit 1
        ;;
esac

echo "=== ape-decoder release ==="
echo "  Current version: $CURRENT"
echo "  New version:     $NEW_VERSION"
echo ""

# --- Pre-flight checks ---
echo "--- Pre-flight checks ---"

# Check for uncommitted or untracked changes
if ! git diff --quiet HEAD 2>/dev/null || [ -n "$(git status --porcelain 2>/dev/null)" ]; then
    echo "ERROR: Uncommitted changes. Commit or stash first."
    echo "  $(git status --short)"
    exit 1
fi

# Run tests
echo "  Running tests..."
cargo test --quiet 2>&1
echo "  Tests: PASS"

# Run clippy
echo "  Running clippy..."
cargo clippy --quiet -- -D warnings 2>&1
echo "  Clippy: PASS"

# Check formatting
echo "  Checking format..."
cargo fmt --check 2>&1
echo "  Format: PASS"

echo ""

# --- Bump version in Cargo.toml ---
echo "--- Bumping version ---"
sed -i "s/^version = \"$CURRENT\"/version = \"$NEW_VERSION\"/" Cargo.toml
echo "  Cargo.toml: $CURRENT -> $NEW_VERSION"

# --- Commit, tag, push ---
echo ""
echo "--- Git operations ---"
git add Cargo.toml
git commit -m "release: v$NEW_VERSION"
echo "  Committed"

git tag -a "v$NEW_VERSION" -m "v$NEW_VERSION"
echo "  Tagged v$NEW_VERSION"

git push origin main
git push origin "v$NEW_VERSION"
echo "  Pushed to origin"

# --- Create GitHub release ---
echo ""
echo "--- GitHub release ---"
gh release create "v$NEW_VERSION" \
    --title "v$NEW_VERSION" \
    --generate-notes
echo "  Release created"

echo ""
echo "=== Released ape-decoder v$NEW_VERSION ==="
echo ""
echo "To publish to crates.io:"
echo "  cargo publish"

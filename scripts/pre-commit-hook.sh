#!/usr/bin/env bash
# Pre-commit hook to check version consistency
# To install: cp scripts/pre-commit-hook.sh .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Running pre-commit checks...${NC}"

# Check if Cargo.toml files are staged
CARGO_TOMLS=$(git diff --cached --name-only --diff-filter=ACM | grep -E 'Cargo\.toml$' || true)

if [ -n "$CARGO_TOMLS" ]; then
    echo -e "${YELLOW}Cargo.toml files changed, checking version consistency...${NC}"

    # Get workspace version
    WORKSPACE_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

    # Check Python package version
    PY_CARGO_VERSION=$(grep '^version = ' crates/simple-agents-py/Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
    PY_PROJECT_VERSION=$(grep '^version = ' crates/simple-agents-py/pyproject.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

    if [ "$WORKSPACE_VERSION" != "$PY_CARGO_VERSION" ] || [ "$WORKSPACE_VERSION" != "$PY_PROJECT_VERSION" ]; then
        echo -e "${RED}✗ Version mismatch detected!${NC}"
        echo "  Workspace: $WORKSPACE_VERSION"
        echo "  Python Cargo.toml: $PY_CARGO_VERSION"
        echo "  Python pyproject.toml: $PY_PROJECT_VERSION"
        echo ""
        echo "Run: ./scripts/sync-versions.sh"
        exit 1
    fi

    echo -e "${GREEN}✓ Version consistency check passed${NC}"
fi

# Check formatting if Rust files are staged
RUST_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.rs$' || true)

if [ -n "$RUST_FILES" ]; then
    echo -e "${YELLOW}Checking Rust formatting...${NC}"
    if ! cargo fmt --all -- --check; then
        echo -e "${RED}✗ Formatting check failed${NC}"
        echo "Run: cargo fmt --all"
        exit 1
    fi
    echo -e "${GREEN}✓ Formatting check passed${NC}"
fi

echo -e "${GREEN}All pre-commit checks passed!${NC}"

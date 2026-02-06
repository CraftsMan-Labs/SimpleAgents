#!/usr/bin/env bash
# Synchronize version numbers across all Cargo.toml and pyproject.toml files
set -euo pipefail

# Get the workspace version from root Cargo.toml
WORKSPACE_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

if [ -z "$WORKSPACE_VERSION" ]; then
    echo "Error: Could not find workspace version in Cargo.toml"
    exit 1
fi

echo "Workspace version: $WORKSPACE_VERSION"
echo ""

# Update Python package version
echo "Updating Python package version..."
sed -i.bak "s/^version = \".*\"/version = \"$WORKSPACE_VERSION\"/" crates/simple-agents-py/Cargo.toml
rm -f crates/simple-agents-py/Cargo.toml.bak

sed -i.bak "s/^version = \".*\"/version = \"$WORKSPACE_VERSION\"/" crates/simple-agents-py/pyproject.toml
rm -f crates/simple-agents-py/pyproject.toml.bak

echo "✓ Python package version updated"
echo ""

# Update examples package versions
if [ -f examples/pyproject.toml ]; then
    echo "Updating examples pyproject version..."
    sed -i.bak "s/^version = \".*\"/version = \"$WORKSPACE_VERSION\"/" examples/pyproject.toml
    rm -f examples/pyproject.toml.bak
    echo "✓ examples/pyproject.toml updated"
    echo ""
fi

if [ -f examples/Cargo.toml ]; then
    echo "Updating examples Cargo version and internal deps..."
    sed -i.bak "s/^version = \".*\"/version = \"$WORKSPACE_VERSION\"/" examples/Cargo.toml
    # Update any path + version deps on our crates (simple-agents-* or simple-agent-type)
    sed -i -E "s/(simple-agents-[a-z-]+ = \{[^}]*version = \")[^\"]+/\1$WORKSPACE_VERSION/" examples/Cargo.toml
    sed -i -E "s/(simple-agent-type = \{[^}]*version = \")[^\"]+/\1$WORKSPACE_VERSION/" examples/Cargo.toml
    rm -f examples/Cargo.toml.bak
    echo "✓ examples/Cargo.toml updated"
    echo ""
fi

# Update internal dependencies in crates that don't use workspace dependencies
echo "Checking internal dependencies..."

# Find all Cargo.toml files in crates
for toml in crates/*/Cargo.toml; do
    crate_name=$(basename "$(dirname "$toml")")

    # Update simple-agents-* and simple-agent-type dependencies
    if grep -q -E 'simple-agents-|simple-agent-type' "$toml"; then
        echo "  Checking $crate_name..."

        # Update each dependency that pins a version (keeps path intact)
        sed -i.bak -E "s/(simple-agents-[a-z-]+ = \{[^}]*version = \")[^\"]+/\1$WORKSPACE_VERSION/" "$toml"
        sed -i -E "s/(simple-agent-type = \{[^}]*version = \")[^\"]+/\1$WORKSPACE_VERSION/" "$toml"
        rm -f "${toml}.bak"
    fi
done

echo "✓ Internal dependencies checked"
echo ""

echo "Version synchronization complete!"
echo ""
echo "Changed files:"
git diff --name-only | grep -E '\.(toml)$' || echo "  (no changes)"

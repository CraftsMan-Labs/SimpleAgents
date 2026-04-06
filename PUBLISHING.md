# Publishing Guide

Quick reference for publishing SimpleAgents packages.

## Prerequisites

1. **Environment Setup**
   - Doppler configured with tokens:
     - `V_PUBLISH_TOKEN` or `UV_PUBLISH_TOKEN` for PyPI
     - `CARGO_REGISTRY_TOKEN` for crates.io
   - Git repository clean (commit all changes)
   - All tests passing

2. **Pre-flight Checklist**
   ```bash
   # Run all checks
   make check-publish

   # This will:
   # - Run tests
   # - Run clippy
   # - Check formatting
   # - Verify package metadata
   # - Dry-run publish
   ```

## Quick Publish Flow

### 1. Bump Version

Choose the appropriate version bump:

```bash
# Patch version (0.1.0 -> 0.1.1) - Bug fixes only
make version-patch

# Minor version (0.1.0 -> 0.2.0) - New features, backward compatible
make version-minor

# Major version (0.1.0 -> 1.0.0) - Breaking changes
make version-major

# Or set a specific version
make version-set VERSION=0.2.0-beta.1
```

### 2. Review Changes

```bash
# Check what changed
git diff

# Verify version is correct
make version-get
```

### 3. Run Checks

```bash
# Run all pre-publish checks (required!)
make check-publish
```

### 4. Commit Version Bump

```bash
VERSION=$(make version-get)
git add .
git commit -m "chore(release): bump version to $VERSION"
```

### 5. Create Tag

```bash
make tag-release
```

### 6. Push to GitHub

```bash
git push origin main --tags
```

### 7. Publish Packages

```bash
# Publish everything
make publish-all

# Or publish individually:
make publish-crates   # Rust crates to crates.io
make publish-python   # Python package to PyPI
```

## Handling Errors

### Crate Name Unavailable

If you get:
```
A crate with the name `X` was recently deleted. Reuse of this name will be available after YYYY-MM-DD
```

**Options:**
1. **Wait** until the date shown
2. **Rename** the crate (see [Renaming Crates](#renaming-crates))
3. **Use dry-run** to test before the actual publish

### Version Already Published

If you get:
```
version X.Y.Z already published
```

You **must** bump the version again:
```bash
make version-patch
git add .
git commit -m "chore(release): bump version to $(make version-get)"
make publish-all
```

### Authentication Failed

If you get 403 errors:
```bash
# Check Doppler secrets
doppler secrets

# Verify tokens are set:
# - V_PUBLISH_TOKEN or UV_PUBLISH_TOKEN (for PyPI)
# - CARGO_REGISTRY_TOKEN (for crates.io)
```

### Rate Limited

If rate limited by crates.io:
```bash
# Wait 10-60 minutes, then retry
# Or publish crates one at a time with delays

for crate in simple-agent-type simple-agents-healing simple-agents-providers simple-agents-core simple-agents-workflow; do
  doppler run -- cargo publish -p $crate
  sleep 60  # Wait between publishes
done
```

## Renaming Crates

If you need to rename a crate due to conflicts:

1. **Choose new name**: e.g., `craftsman-simple-agent-type`

2. **Update Cargo.toml**:
   ```toml
   [package]
   name = "craftsman-simple-agent-type"  # Changed
   ```

3. **Update all dependencies**:
   ```bash
   # Find all references
   grep -r "simple-agent-type" crates/

   # Update in each Cargo.toml:
   craftsman-simple-agent-type = { path = "../simple-agent-type" }
   ```

4. **Update imports** in Rust code:
   ```rust
   use craftsman_simple_agent_type::prelude::*;
   ```

5. **Update Makefile**:
   ```makefile
   PUBLISH_CRATES = craftsman-simple-agent-type ...
   ```

## Rollback

If you need to rollback a failed release:

### Before Publishing

```bash
# Reset version bump
git reset --hard HEAD~1

# Or if you've pushed
git revert HEAD
git push origin main
```

### After Publishing (Partial Failure)

You **cannot** unpublish or delete versions from crates.io or PyPI.

Options:
1. **Publish remaining crates** after fixing issues
2. **Yank the version** (makes it unavailable for new projects):
   ```bash
   cargo yank --version X.Y.Z simple-agent-type
   ```
3. **Publish a fix version** (bump patch and republish)

## CI/CD Publishing (Future)

For automated releases via GitHub Actions:

1. Push a tag: `git push origin v0.1.0`
2. GitHub Actions workflow triggers
3. Runs tests and checks
4. Publishes to registries automatically

See `.github/workflows/release.yml` (when implemented).

## Troubleshooting

### Check Current Version

```bash
make version-get
```

### View Next Version

```bash
make version-next-patch  # Shows next patch version
make version-next-minor  # Shows next minor version
make version-next-major  # Shows next major version
```

### Dry-run Publish

Always test publishing before doing it for real:

```bash
# Test Rust crates
make publish-crates-dry

# Test Python package
make publish-python-dry
```

### Verify Package Contents

```bash
# Check what files will be included
cargo package --list -p simple-agent-type

# Build the package locally
cargo package -p simple-agent-type
```

### Check Token Permissions

```bash
# For PyPI
doppler secrets get UV_PUBLISH_TOKEN

# For crates.io
doppler secrets get CARGO_REGISTRY_TOKEN
```

## Best Practices

1. **Always dry-run first**
   - Never publish without testing with `--dry-run`
   - Run `make check-publish` before every release

2. **Use conventional commits**
   - Makes it easier to determine version bump type
   - Example: `feat:` = minor, `fix:` = patch, `feat!:` or `BREAKING CHANGE:` = major

3. **Document breaking changes**
   - Add CHANGELOG.md entries
   - Clearly mark breaking changes

4. **Test before release**
   - All tests must pass
   - No clippy warnings
   - Code must be formatted

5. **Tag releases**
   - Always create git tags for releases
   - Use annotated tags: `git tag -a v0.1.0 -m "Release 0.1.0"`

6. **Coordinate multi-package releases**
   - Ensure all dependent packages are compatible
   - Publish in dependency order (types before providers)

7. **Wait between publishes**
   - Don't spam the registry
   - If publishing manually, wait 30-60s between crates

## Emergency Contacts

If you encounter issues:

- **Crates.io Help**: https://crates.io/policies
- **PyPI Help**: https://pypi.org/help/
- **GitHub Issues**: https://github.com/yourusername/SimpleAgents/issues

## Quick Command Reference

```bash
# Version management
make version-get              # Show current version
make version-patch            # 0.1.0 -> 0.1.1
make version-minor            # 0.1.0 -> 0.2.0
make version-major            # 0.1.0 -> 1.0.0
make version-set VERSION=X    # Set specific version

# Pre-publish checks
make check-publish            # Run all checks
make publish-crates-dry       # Test Rust publish
make publish-python-dry       # Test Python publish

# Publishing
make publish-crates           # Publish Rust crates
make publish-python           # Publish Python package
make publish-all              # Publish everything

# Git operations
make tag-release              # Create version tag
git push origin main --tags   # Push with tags

# Utilities
./scripts/sync-versions.sh    # Sync versions across files
```

# Versioning Guide

This document describes the versioning strategy and release process for SimpleAgents.

## Semantic Versioning

We follow [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR** version: Incompatible API changes
- **MINOR** version: New functionality in a backward compatible manner
- **PATCH** version: Backward compatible bug fixes

### Pre-release Versions

For pre-release versions, append:
- `-alpha.N` for alpha releases (unstable, experimental)
- `-beta.N` for beta releases (feature complete, testing)
- `-rc.N` for release candidates (final testing)

Example: `0.2.0-alpha.1`, `1.0.0-beta.2`, `1.0.0-rc.1`

## Version Management

### Workspace Version

All crates share the same version defined in the root `Cargo.toml`:

```toml
[workspace.package]
version = "0.1.0"
```

Individual crates inherit this version:

```toml
[package]
version.workspace = true
```

In-tree crates listed under `[workspace.dependencies]` use **`version` and `path` together**: the path keeps local builds on workspace sources, and the version is what remains after `cargo publish` strips `path` (crates.io rejects `workspace = true` dependencies with no version). `make version-sync` keeps those versions aligned with `[workspace.package].version`. `./scripts/verify-workspace-versions.sh` (also run at the end of `make version-sync` and in CI) enforces that alignment and runs `cargo metadata --locked`.

### Bumping Versions

Use the Makefile commands:

```bash
# Bump patch version (0.1.0 -> 0.1.1)
make version-patch

# Bump minor version (0.1.0 -> 0.2.0)
make version-minor

# Bump major version (0.1.0 -> 1.0.0)
make version-major

# Set specific version
make version-set VERSION=0.2.0-alpha.1
```

## Publishing Workflow

### Pre-flight Checks

Before publishing, always run checks:

```bash
# Run all pre-publish checks
make check-publish

# This will:
# - Run all tests
# - Run clippy
# - Check formatting
# - Verify crate metadata
# - Dry-run publish
```

### Dry-run Publishing

Test publishing without actually uploading:

```bash
# Dry-run Rust crates
make publish-crates-dry

# Dry-run Python package
make publish-python-dry
```

### Actual Publishing

Only publish after dry-run succeeds:

```bash
# Publish Rust crates
make publish-crates

# Publish Python package
make publish-python

# Publish everything
make publish-all
```

### Full Release Process

1. **Update version**:
   ```bash
   make version-minor  # or version-patch, version-major
   ```

2. **Update CHANGELOG** (if exists):
   - Add release notes for the new version
   - Document breaking changes, new features, bug fixes

3. **Run checks**:
   ```bash
   make check-publish
   ```

4. **Commit version bump**:
   ```bash
   git add .
   git commit -m "chore(release): bump version to $(make version-get)"
   ```

5. **Create git tag**:
   ```bash
   make tag-release
   ```

6. **Push changes**:
   ```bash
   git push origin main --tags
   ```

7. **Publish packages**:
   ```bash
   make publish-all
   ```

## Handling Publishing Errors

### Crate Name Already Taken

If you get an error like:
```
crate name already taken
```

Options:
1. **Wait**: If recently deleted, wait for the timeout period
2. **Rename**: Choose a different crate name
3. **Prefix**: Add an org prefix (e.g., `craftsman-simple-agent-type`)

To rename a crate:
```bash
# Edit Cargo.toml files
# Update package name
name = "craftsman-simple-agent-type"

# Update all dependencies in other crates
simple-agent-type = { path = "../simple-agent-type" }
# becomes:
craftsman-simple-agent-type = { path = "../simple-agent-type" }
```

### Version Already Published

If you get:
```
version X.Y.Z already published
```

You **cannot** republish the same version. You must:
1. Bump the version: `make version-patch`
2. Commit the change
3. Try publishing again

### Authentication Errors

If you get 403 errors:
1. Check your token is set in Doppler
2. Verify token has publish permissions
3. Check token hasn't expired

### Rate Limiting

If you get rate limited:
1. Wait for the rate limit to reset (usually 10-60 minutes)
2. Use `cargo publish --no-verify` to skip build step
3. Stagger publishing of multiple crates

## Git Tagging

### Creating Tags

Tags are automatically created with:
```bash
make tag-release
```

This creates an annotated tag like `v0.1.0`.

### Manual Tagging

```bash
# Create annotated tag
git tag -a v0.1.0 -m "Release version 0.1.0"

# Push tag
git push origin v0.1.0

# Push all tags
git push origin --tags
```

### Tag Naming Convention

- Rust crates: `v{VERSION}` (e.g., `v0.1.0`)
- Python package: `py-v{VERSION}` (e.g., `py-v0.1.0`)
- Node package: `node-v{VERSION}` (e.g., `node-v0.1.0`)

## CI/CD Integration

For automated releases, use GitHub Actions:

1. Create `.github/workflows/release.yml`
2. Trigger on tag push: `v*`
3. Run checks and publish automatically

See `.github/workflows/release.yml` for details (if implemented).

## Troubleshooting

### Check Current Version

```bash
make version-get
```

### Verify Crate Metadata

```bash
cargo package --list -p simple-agent-type
```

### Test Publishing Locally

```bash
# Create a test registry
mkdir -p /tmp/my-registry
cargo publish --dry-run --registry test
```

## Best Practices

1. **Always dry-run first**: Never publish without `--dry-run` first
2. **Test before release**: Run full test suite before bumping version
3. **Document changes**: Update CHANGELOG or release notes
4. **Use conventional commits**: Makes changelog generation easier
5. **Tag releases**: Always create git tags for releases
6. **Coordinate releases**: When publishing multiple packages, ensure compatibility
7. **Publish in order**: Publish dependencies before dependents

## Dependencies Between Crates

Publishing order for SimpleAgents:

1. `simple-agent-type` (no internal deps)
2. `simple-agents-cache` (no internal deps)
3. `simple-agents-macros` (no internal deps)
4. `simple-agents-healing` (depends on types)
5. `simple-agents-router` (depends on types)
6. `simple-agents-providers` (depends on types, cache, healing)
7. `simple-agents-core` (depends on everything)
8. `simple-agents-ffi` (depends on core)

The Makefile respects this order automatically.

## Version Compatibility

### Rust Crates

When updating dependencies:
- `0.x.y` versions: Breaking changes allowed in minor versions
- `1.x.y` versions: Breaking changes only in major versions

### Python Package

Python package version should match Rust workspace version for consistency.

## Quick Reference

```bash
# Check current version
make version-get

# Bump version
make version-patch    # 0.1.0 -> 0.1.1
make version-minor    # 0.1.0 -> 0.2.0
make version-major    # 0.1.0 -> 1.0.0

# Run pre-publish checks
make check-publish

# Dry-run publish
make publish-crates-dry
make publish-python-dry

# Actually publish
make publish-crates
make publish-python
make publish-all

# Tag release
make tag-release

# Get next version
make version-next-patch
make version-next-minor
make version-next-major
```

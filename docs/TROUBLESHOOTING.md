# Troubleshooting

Common issues and fast checks for core and binding workflows.

## Environment and Credential Issues

- Missing env vars for live tests:
  - Set `PROVIDER`, `CUSTOM_API_MODEL`, `CUSTOM_API_KEY`.
  - If needed, set `CUSTOM_API_BASE`.
- Node tests skip live coverage without env by design.
- Python tests skip live streaming checks without env by design.
- Go live tests skip when provider/model/key are not set.

## Binding Contract Failures

- If fixture-based tests fail, first validate `parity-fixtures/binding_contract.json`.
- Run all contract gates locally:

```bash
./scripts/run-binding-contracts.sh
```

- Typical failure patterns:
  - Symbol missing in generated declarations or stubs.
  - Binding API changed without fixture updates.
  - Streaming event shape changed without parity updates.

## FFI and Go Linking Issues

- Ensure the FFI library builds before Go tests.
- Use the repository `make` target (it wires include/link flags correctly):

```bash
make test-go-bindings
```

## Python Test Runtime Issues

- Use `uv` and include pytest dependency:

```bash
UV_CACHE_DIR=$(pwd)/.uv-cache uv run --directory crates/simple-agents-py --with "pytest>=8.0" pytest
```

- If lockfile churn appears during local runs, avoid committing unrelated lockfile updates.

## Node Binding Build/Test Issues

- Rebuild declarations and native module:

```bash
npm --prefix crates/simple-agents-napi ci
npm --prefix crates/simple-agents-napi test
```

- Declaration parity is validated by fixture-backed tests and TypeScript declaration checks.

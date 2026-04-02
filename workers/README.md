# Multi-language workflow workers

Phase 4 adds standalone worker runtimes that expose the shared gRPC worker protocol at:

- `workers/python/worker.py`
- `workers/go/worker.go`
- `workers/typescript/worker.ts`

Each runtime provides:

- `Health` RPC for liveness and coarse status
- `Execute` RPC for tool/llm-like request execution

`ExecuteRequest` supports both typed payloads (`llm_payload` / `tool_payload`) and the
legacy `payload_json` fallback for backward compatibility during rollout.

Smoke scripts are included in each language subdirectory to validate local startup and one execute roundtrip.

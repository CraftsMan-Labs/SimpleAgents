# PLAN_OPTIMISATIONS

This plan tracks execution of all open findings in `OPTIMISATION.md`.

## Execution Plan

- [x] **P1 / H2:** Guard retry helpers against `max_attempts == 0` and remove panic paths.
  - [x] Add config-level validation for retry attempt count.
  - [x] Replace `unwrap()` terminal error paths with typed errors.
  - [x] Add regression tests for zero and one-attempt behavior.

- [x] **P2 / H1:** Enforce unique provider names in builder path.
  - [x] Validate uniqueness in `SimpleAgentsClientBuilder::build()`.
  - [x] Return `SimpleAgentsError::Config` on duplicates.
  - [x] Add tests for duplicate `.with_provider()` and `.with_providers()` input.

- [x] **P3 / M2:** Honor retry settings for single-worker pools.
  - [x] Compute attempts as `max_retries + 1` independent of pool size.
  - [x] Retry same worker when pool size is one.
  - [x] Add tests for transient failure retry behavior.

- [x] **P4 / M3:** Remove `.await` while holding pool-wide mutex in worker selection.
  - [x] Refactor selection loop to snapshot state under lock.
  - [x] Perform async health/hook calls outside lock scope.
  - [x] Add/adjust tests for correctness under contention.

- [x] **P5 / M4:** Wire `Retry-After` end-to-end.
  - [x] Extend parser to support seconds and HTTP-date.
  - [x] Pass parsed values through provider 429 error mapping.
  - [x] Use parsed values in retry/backoff behavior.
  - [x] Add tests for both header formats.

- [x] **P6 / M1:** Eliminate Go worker proto drift risk.
  - [x] Load descriptor from `worker.proto` as source of truth at runtime.
  - [x] Remove manual descriptor shape construction.
  - [x] Validate worker build/tests.

- [x] **P7 / M5:** Normalize provider transport defaults.
  - [x] Prefer negotiated HTTP transport by default.
  - [x] Keep forced HTTP/2 prior-knowledge as explicit opt-in.
  - [x] Update docs/comments where behavior changed.

- [x] **P8 / L1:** Fix README/workspace version drift.
  - [x] Update stale version references in docs.
  - [x] Add CI guard to detect drift.

- [x] **P9 / L2:** Add TypeScript worker runtime configurability.
  - [x] Add `--worker-id` and `--listen` CLI/env support.
  - [x] Thread configured values into health/execute responses.

- [x] **P10 / Validation:** Run targeted and workspace checks.
  - [x] Run crate-level tests for changed areas.
  - [x] Run workspace checks where feasible.
  - [x] Update `OPTIMISATION.md` statuses to done.

## Notes

- Tasks are checked off in order as each fix lands.
- If a task is partially complete, its sub-items are marked first.

# Workflow Release Checklist

Use this checklist before cutting a release that includes workflow runtime changes.

## 1. Validation and Tests

- [ ] `cargo test -p simple-agents-workflow`
- [ ] `cargo check --workspace`
- [ ] `./scripts/run-binding-contracts.sh`
- [ ] `./scripts/run-binding-tests-layered.sh`

## 2. Performance and Regression Gates

- [ ] `cargo bench -p simple-agents-workflow --bench runtime_benchmarks -- --sample-size 10`
- [ ] Confirm concurrency regression guard passes (`WORKFLOW_BENCH_*` thresholds)
- [ ] Review benchmark deltas before release tagging

## 3. Security Hardening Verification

- [ ] Expression complexity limits validated
- [ ] Runtime scope/fan-out limits validated
- [ ] Worker request contract enforcement validated
- [ ] YAML file loader rejection checks validated (`.yaml/.yml`, size/depth/path policy)
- [ ] Secret handling policy reviewed for workflow examples/docs

## 4. Docs and Examples

- [ ] Update `docs/WORKFLOW_PERFORMANCE.md` if benchmark strategy changed
- [ ] Update `docs/WORKFLOW_SECURITY.md` for contract changes
- [ ] Ensure `workflow-engine-research/examples/` has current coverage and schema examples
- [ ] Update `CHANGELOG.md` with workflow section for the release (heading format: `## [x.y.z] - YYYY-MM-DD`)

## 5. Final Release Hygiene

- [ ] Verify no credentials in committed files
- [ ] Verify CI workflows include workflow bench and binding gates
- [ ] Verify OTLP HTTP/protobuf trace configuration tests are green
- [ ] Tag release after all checks are green

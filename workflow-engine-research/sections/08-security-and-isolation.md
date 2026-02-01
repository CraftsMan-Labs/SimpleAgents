# Security and Isolation

## Recommended Repo
- https://github.com/open-policy-agent/opa

## Why This Repo
- Policy engine widely used for access control and authorization decisions.
- Maps to graph-based policies and capability enforcement.

## Pros
- Policy-as-code with a mature ecosystem.
- Fits graph-level access policies and capability checks.

## Cons
- Adds a separate policy language and runtime dependency.
- Needs careful integration to avoid latency overhead.

## What We Want To Build From This
- Policy evaluation for graph/node permissions and resource access.
- Capability tokens enforced in the Rust core.

## Why
- Security boundaries must be enforceable regardless of language workers.

## Sources
- https://www.openpolicyagent.org/docs
- https://www.cncf.io/projects/open-policy-agent-opa/

## Notes
- Use OPA patterns for policy definitions; evaluate additional sandboxing options separately.

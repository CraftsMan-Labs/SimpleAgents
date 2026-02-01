# Deployment and Performance Targets

## Recommended Repo
- https://github.com/aws/aws-lambda-rust-runtime

## Why This Repo
- Canonical Rust serverless runtime for cold-start and deployment constraints.
- Useful reference for bundle size and init-time tradeoffs.

## Pros
- Real-world guidance for Rust serverless deployments.
- Helps quantify cold-start and init-time constraints.

## Cons
- Tied to AWS Lambda constraints; may not generalize to all deployments.
- Serverless environment limits can be restrictive for long-lived workflows.

## What We Want To Build From This
- Minimal cold-start strategy (small artifacts, warm pools).
- Clear init-time and memory footprint targets per worker.

## Why
- Fast production rollout depends on predictable startup and resource use.

## Sources
- https://docs.aws.amazon.com/lambda/latest/dg/lambda-rust.html
- https://aws.amazon.com/blogs/opensource/rust-runtime-for-aws-lambda/

## Notes
- Use Lambda runtime patterns to inform cold-start minimization and deployment packaging.

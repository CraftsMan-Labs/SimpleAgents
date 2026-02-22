---
description: Group changes into clean conventional commits and push
agent: build
---
Create clean, meaningful git commits for the current branch and push to remote.

Optional user intent/context: $ARGUMENTS

Execution rules:
1) Inspect current changes and recent commit style using:
   - git status
   - git diff (staged + unstaged)
   - git log --oneline -15
2) Group changes into logically coherent commits (feature/fix/docs/chore/test/refactor) using Conventional Commit style.
3) Do not include unrelated local changes; skip files that are clearly out of scope.
4) Never commit secrets/credentials/tokens/.env files.
5) Use concise commit messages focused on intent and impact.
6) Run minimal relevant verification before final push (at least targeted tests for touched area when available).
7) Push the branch to remote (set upstream if needed).

Output format:
- Brief summary of grouped commits created
- Commit SHAs and messages
- Verification commands run and results
- Final push status and remote branch

---
name: commit-clean
description: Groups local git changes into clean Conventional Commits, runs minimal relevant verification, and pushes the branch safely. Use when the user asks to create organized commits, clean up commit history on a branch, or commit and push related changes.
---

# Commit Clean

Create clean, meaningful commits for the current branch and push to remote.

## When to use

Use this skill when the user asks to:

- create commits from current local changes
- split work into logical commits
- follow Conventional Commit message style
- push the current branch after committing

## Workflow

1. Inspect repository state:
   - `git status`
   - `git diff` (staged + unstaged)
   - `git log --oneline -15`
2. Group changes into coherent commits by intent:
   - `feat`, `fix`, `docs`, `chore`, `test`, `refactor`
3. Keep scope clean:
   - exclude unrelated local changes
   - skip out-of-scope files
4. Protect secrets:
   - never commit credentials, tokens, `.env`, or secret-like files
5. Write concise commit messages:
   - focus on intent and impact
   - follow Conventional Commit style
6. Run minimal verification for touched areas:
   - at least targeted tests/checks when available
7. Push branch to remote:
   - set upstream if needed (`git push -u origin <branch>`)

## Safety rules

- Do not change global git config.
- Do not use destructive git commands.
- Do not force-push unless the user explicitly requests it.
- Do not commit files that look like secrets even if accidentally staged.

## Output format

- Brief summary of grouped commits created
- Commit SHAs and messages
- Verification commands run and results
- Final push status and remote branch

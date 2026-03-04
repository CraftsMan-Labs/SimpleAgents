# Docs Modernization Plan (CocoIndex-Level DX)

## Objective

Upgrade SimpleAgents documentation to match the clarity, navigability, and developer experience quality seen in CocoIndex docs while keeping the current VitePress stack.

## Why this plan

The CocoIndex docs quality comes from repeatable patterns:

1. Strong information architecture by user intent
2. Predictable page structure and scanning flow
3. Reusable docs UI patterns (buttons/cards/callouts)
4. Visual explanations for complex systems
5. Docs quality gates in CI

SimpleAgents already has a good baseline (`docs/.vitepress/config.mjs`, `docs/DOCS_MAP.md`, `docs/DOCS_STANDARDS.md`), so this plan focuses on consistency, readability, and docs operations.

## Success criteria

- All top-level docs pages follow a shared template
- Core journey pages include outcome-first intro, prerequisites, runnable snippets, troubleshooting, and next steps
- Sidebar and docs map expose three explicit paths: New User, Integrator, Contributor/Maintainer
- Architecture/workflow-heavy docs include diagrams
- CI validates docs build and link health
- Updated pages include metadata discipline (title + description, and optional audience/last reviewed)

## Non-goals

- No migration from VitePress to Docusaurus
- No full visual rebrand before structural quality is improved
- No all-at-once rewrite of every page

## Gap analysis summary

Compared with CocoIndex docs, current gaps are:

1. Template inconsistency across pages
2. Limited reusable docs components and CTA patterns
3. Not enough visual models in complex guides
4. Uneven related/next-step cross-linking
5. Incomplete docs-specific CI quality gates

## Execution phases

### Phase 0 - Standards lock (Day 1)

Goal: Define one documentation contract before broad edits.

Tasks:

- Expand `docs/DOCS_STANDARDS.md` with a canonical page template:
  - What this page gives you
  - Prerequisites
  - Quick path
  - Concept model / deep dive
  - Troubleshooting
  - Next steps
- Define metadata expectations per page:
  - `title`
  - `description`
  - `audience` (optional)
  - `last_reviewed` (optional)
- Add practical writing constraints:
  - Outcome-first opening
  - Short paragraphs
  - Numbered procedures
  - At least one runnable snippet per major task section

Deliverable:

- Updated standards doc with copyable page template

### Phase 1 - Navigation and journey refinement (Day 1-2)

Goal: Make path-to-answer obvious in two clicks or fewer.

Tasks:

- Align sidebar structure in `docs/.vitepress/config.mjs` with journey groups:
  - Start Here
  - Build and Operate
  - Workflows
  - Bindings
  - Architecture and Internals
  - Reference
- Update `docs/DOCS_MAP.md` to mirror sidebar groupings and reading paths
- Add explicit role-oriented entry links (New User, Integrator, Contributor)

Deliverable:

- Sidebar/docs map parity with role-based reading paths

### Phase 2 - Reusable docs UI patterns (Day 2-3)

Goal: Improve consistency and scanning without changing platform.

Tasks:

- Add reusable patterns for:
  - CTA links/buttons
  - Prerequisite checklist blocks
  - Related docs blocks
  - Path selection cards
- Standardize admonition usage (`tip`, `warning`, `info`)
- Centralize high-frequency snippets (setup/build/test commands)

Deliverable:

- Documented reusable docs patterns used by at least core pages

### Phase 3 - Core page rewrites (Day 3-6)

Goal: Upgrade highest-traffic pages first.

Priority 1 pages:

- `docs/QUICKSTART.md`
- `docs/USAGE.md`
- `docs/ARCHITECTURE.md`
- `docs/YAML_WORKFLOW_SYSTEM.md`
- `docs/WORKFLOW_DEBUGGING.md`

Priority 2 pages:

- `docs/WORKFLOW_PERFORMANCE.md`
- `docs/WORKFLOW_SECURITY.md`
- `docs/API.md`
- `docs/DEVELOPMENT.md`
- `docs/TROUBLESHOOTING.md`

Per-page rewrite checklist:

- Outcome-first intro and audience fit
- Quick-start runnable path
- One conceptual model (diagram/table)
- Failure modes and concrete fixes
- Next steps links

Deliverable:

- Core journey docs follow consistent, high-DX structure

### Phase 4 - Visual model pass (Day 5-7, parallel)

Goal: Reduce cognitive load for complex topics.

Tasks:

- Add or refine diagrams in:
  - `docs/ARCHITECTURE.md`
  - `docs/YAML_WORKFLOW_SYSTEM.md`
  - `docs/WORKFLOW_DEBUGGING.md`
  - `docs/WORKFLOW_PERFORMANCE.md`
- Prefer Mermaid for maintainability and easy updates

Deliverable:

- Each architecture/workflow-heavy guide has at least one clear visual model

### Phase 5 - Docs CI quality gates (Day 6-7)

Goal: Prevent docs regressions.

Tasks:

- Ensure docs build runs on docs-touching PRs
- Add link validation checks (internal and key external links)
- Add markdown lint/style checks
- Add docs completeness checks in PR workflow/checklist

Deliverable:

- Repeatable docs quality gate for every PR

### Phase 6 - Final polish and scorecard (Day 7)

Goal: Measure quality and lock the process.

Tasks:

- Final pass for stale snippets, broken links, duplicate guidance
- Publish a simple docs scorecard:
  - Template compliance
  - Pages with troubleshooting
  - Pages with next steps
  - Link-check pass rate

Deliverable:

- Documented quality snapshot and maintenance workflow

## Suggested ownership

1. Information architecture
   - Files: `docs/.vitepress/config.mjs`, `docs/DOCS_MAP.md`
2. Template and standards
   - File: `docs/DOCS_STANDARDS.md`
3. Core guide rewrites
   - Files: priority pages listed above
4. Visual model layer
   - Files: architecture/workflow pages
5. Docs CI gates
   - Files: CI workflow and docs scripts

## Risks and mitigations

- Inconsistent writing across contributors
  - Mitigation: strict template + examples + PR checklist
- Drift between docs and APIs
  - Mitigation: use tested snippets from examples and keep docs updates in same PR as behavior changes
- Rewrite churn and navigation confusion
  - Mitigation: phase rollout by traffic and maintain stable links

## Immediate next actions

1. Lock standards and template structure
2. Align sidebar and docs map
3. Rewrite Priority 1 pages using the template
4. Add CI checks before broad rewrite completion

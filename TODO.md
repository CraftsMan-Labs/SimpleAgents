# Active TODO

Date: 2026-03-04
Scope: Active execution tasks (remaining work only)

## Status values

- `pending`
- `in_progress`
- `completed`
- `blocked`

## Open tasks

| ID | Task | Why this is needed | Expected outcome | Status |
|---|---|---|---|---|
| P1 | Freeze parity matrix and acceptance criteria | Work needs one source-of-truth so parity is testable and reviewable | Matrix mapping Python APIs/features to Node/Go status with explicit P0/P1 priorities and acceptance checks | pending |
| X6 | Update docs and verify gates | Users and maintainers need clear usage + reproducible checks | Docs include copy-paste option examples and all relevant tests pass | pending |
| D1 | Define docs modernization plan | Team needs an explicit execution blueprint modeled after high-DX CocoIndex docs patterns | `plan.md` exists with phased strategy, success criteria, risks, and deliverables | completed |
| D2 | Standardize docs structure and writing contract | Contributors need one repeatable page format to improve scanability and consistency | `docs/DOCS_STANDARDS.md` contains required template, metadata contract, and review checklist | completed |
| D3 | Align navigation and journey paths | Users need fast route-to-answer by role and intent | Sidebar and `docs/DOCS_MAP.md` reflect New User, Integrator, Contributor pathways with clear progression | completed |
| D4 | Refresh priority guides with high-DX format | Highest-traffic pages need outcome-first structure, runnable steps, and troubleshooting | Priority docs (`QUICKSTART`, `USAGE`, architecture/workflow guides) updated to shared template | completed |
| D5 | Add docs quality gates | Docs regressions should be blocked automatically | CI enforces docs build and link/style checks for docs-touching PRs | completed |

# Documentation Standards

This page defines the required writing and structure contract for all SimpleAgents docs.

## Goals

- Make first success possible in under 10 minutes for new users.
- Keep all guides predictable to scan and easy to maintain.
- Keep docs aligned with real behavior in code and examples.

## Required Page Template

All practical guides should follow this structure in order:

1. **What this page gives you**
   - 2-4 lines, outcome-first.
   - State audience (`New User`, `Integrator`, or `Contributor`) when helpful.
2. **Prerequisites**
   - Required tools, versions, and environment variables.
3. **Quick path**
   - Numbered steps that produce a working outcome.
   - Use copy-paste runnable commands.
4. **Deep dive / concept model**
   - Explain mental model, design tradeoffs, and boundaries.
   - Include one diagram or table for complex systems.
5. **Troubleshooting**
   - Common failures with direct fixes.
6. **Next steps**
   - 3-6 links to the next most relevant docs.

## Metadata Contract

Every page should include at least:

- `title`
- `description`

Optional fields when useful:

- `audience`: `new-user`, `integrator`, `contributor`
- `last_reviewed`: `YYYY-MM-DD`

## Writing Rules

- Lead with outcome, not background.
- Keep headings task-oriented (`Install`, `Run`, `Debug`, `Verify`).
- Keep paragraphs short (2-4 lines).
- Prefer numbered steps for procedures.
- Include at least one runnable snippet per major task section.
- Use explicit statements over vague language.

## Formatting and Accessibility Rules

- Use descriptive link text (`Workflow Debugging UX`) instead of `click here`.
- Keep heading hierarchy ordered (`H1 -> H2 -> H3`).
- Use tables only for truly tabular data.
- Do not rely on color alone to communicate meaning.
- Use fenced code blocks with language labels.

## Navigation and Cross-linking Rules

- Every practical page must include `Next steps`.
- Every high-level page should link to implementation-level docs.
- Keep docs map and sidebar in sync.
- Favor task links over category links when possible.

## Accuracy Rules

- Keep versions and APIs aligned with current workspace state.
- Prefer snippets sourced from tested examples when available.
- Update docs in the same PR as behavior changes.
- Avoid pseudocode for setup commands unless explicitly marked.

## Reusable Section Snippets

Use these blocks to keep style consistent.

### Opening pattern

```md
This guide shows you how to <outcome> using <surface>.
By the end, you will have <concrete result>.
```

### Prerequisites pattern

```md
## Prerequisites

- Rust toolchain: `stable`
- Required env var: `OPENAI_API_KEY`
- Optional tooling: `docker`, `make`
```

### Troubleshooting pattern

````md
## Troubleshooting

### <Symptom>

<Likely cause>.

Fix:

```sh
<command>
```
````

## Review Checklist

- Is the first successful path clear in under 60 seconds?
- Can users find language-specific docs in two clicks or fewer?
- Are commands copy-paste runnable?
- Are internal links valid?
- Does the page end with clear next steps?

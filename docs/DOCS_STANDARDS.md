# Documentation Standards

This page defines how docs should be written to stay easy to scan, accessible, and maintainable.

## Writing Rules

- Lead with the outcome in the first sentence.
- Keep headings task-oriented (`Install`, `Run`, `Troubleshoot`).
- Keep paragraphs short (2-4 lines when possible).
- Prefer numbered steps for procedures.
- Include one working snippet per major section.

## Accessibility Rules

- Use descriptive link text (`Usage Guide`) instead of `click here`.
- Keep heading hierarchy in order (`H1 -> H2 -> H3`).
- Use tables only for truly tabular data.
- Do not rely on color alone to communicate meaning.
- Use fenced code blocks with language labels.

## Navigation Rules

- Every page should link to at least one previous and one next page (or the docs map).
- Add a short `Next steps` section at the bottom of practical guides.
- Link from high-level docs to exact implementation docs.

## Accuracy Rules

- Keep versioned snippets aligned with current workspace versions.
- Prefer tested snippets from `examples/` when available.
- Update docs in the same PR as behavior changes.

## Suggested Review Checklist

- Is the first successful path clear in under 60 seconds?
- Can a user find language-specific docs in under 2 clicks?
- Are all commands copy-paste runnable?
- Are internal links valid?
- Is there a clear "what to read next" section?

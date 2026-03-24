# Changesets

This directory contains changeset files that describe changes made to the codebase.

## Adding a changeset

When making a change that should be included in a release, add a changeset file to your PR:

```bash
npx changeset
```

Or create a file manually in this directory:

```markdown
---
"@spear-ai/webway": patch
---

Description of what changed.
```

Bump types:
- `patch` — bug fix
- `minor` — new feature, backwards compatible
- `major` — breaking change

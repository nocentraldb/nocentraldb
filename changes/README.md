# Change fragments

Every pull request that changes the product adds one fragment here: `changes/<slug>.md`. The file name is the slug; it matches `^[a-z0-9][a-z0-9-]*$` (e.g. `add-sync-retry.md`). This README is the writer's guide and is not a fragment. The full rules, including which paths need no fragment, are in [docs/release-management.md](../docs/release-management.md#writing-a-change-fragment).

Check your fragments with `node scripts/release/check-fragments.mjs` (part of `pnpm gates`, as the task `repo:fragments`).

| field | values | required |
|---|---|---|
| `type` | `breaking`, `feature`, `fix`, `deprecation`, `security`, `internal` | yes |
| `tickets` | nxf ids, e.g. `[mt08.1qaz]` | yes |
| `issues` | GitHub issue numbers of this repository, e.g. `[42, 57]`; each a positive integer, none twice. The release notes link them. | no |
| `introduced` | `cycle` (the bug only existed since the last release) or the released version `X.Y.Z` that introduced it | yes for `fix`, forbidden otherwise |
| `components` | non-empty list from `core`, `sdk-rust`, `sdk-ts`, `cli`, `relay` | yes |

A template (here for a `feature` that closes two GitHub issues; leave `issues` out when there are none); the body's level-2 headings are the sections, exactly those of the type:

```md
---
type: feature
tickets: [mt08.1qaz]
issues: [42, 57]
components: [core]
---
## Problem
What a user could not do, or did badly.

## Use
How to use it, with a short example.
```

Sections by type:

- `breaking`: `## Problem`, `## Use`, `## Migration`
- `feature`: `## Problem`, `## Use`
- `fix` (with `introduced:`): `## Symptom`, `## When`, `## Cause`, `## Fix`
- `deprecation`: `## Deprecated`, `## Replacement`, `## Removed in`
- `security`: `## Description`, `## Advisory` (`pending` until the advisory is published)
- `internal`: no sections, one line of body. Never shown in curated notes.

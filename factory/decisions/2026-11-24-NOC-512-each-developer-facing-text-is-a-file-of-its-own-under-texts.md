# 2026-11-24 · NOC-512 · Each developer-facing text is a file of its own, texts/<kind>/<name>.json

- Decided by: coding agent, NOC-512, within the owner's decision of 2026-11-24 to split the files every feature appends to
- Why: in one sorted `texts/texts.json`, a feature's new texts share a prefix and land side by side, so two pull requests that each add a text conflicted (8 of the last 50 merged pull requests changed it). One file per text, holding its entry as `JSON.stringify(entry, null, 2)` writes it, makes two new texts two new files. The entry keeps its `id` and `kind`, and the gate checks that its path matches its id, so a new file's diff shows the whole entry. `texts/README.md` holds what the file's `$comment` said. `pnpm texts` reads a release tag's folder, or its `texts.json` for a tag from before the split; the owner still reviews the texts from git.
- Instead of: one file per kind or per area, whose sorted entries would still meet when one feature's texts share a prefix; or a file per text without `id` and `kind`, which a reader of a new file's diff would have to work out from its path.
- Topics: texts, gates, release

# Vendored: tasteskill v2 (`design-taste-frontend`)

- Source: https://github.com/Leonxlnx/taste-skill, folder `skills/taste-skill`, MIT (`LICENSE` beside this file, copied from the repository root).
- Pinned at repository commit `f624f37a8c650b9cc550253104925efe4c8de246` (2026-10-14); the folder's tree hash there is `d11cb9568d2f27401ce1f48217721e69ee32637f`, last changed by `9da1763c950baed17a69158a5f4c76f8f85bf49b` (2026-10-12).
- Copied unchanged with `git archive`; `diff -r` against that tree is empty. This file and `LICENSE` are the only additions.
- Not copied: the variants (minimalist, soft, brutalist, redesign, brandkit, image-to-code, imagegen-*, stitch, output, gpt-tasteskill, v1, v2-experimental).

Read before pinning (2026-10-16), and what they do:

| Script | Network | Writes |
| --- | --- | --- |
| `draw.mjs` | none | `.taste-skill/direction.json` (or `--out`); history in `~/.taste-skill/history.json` (or `$TASTE_SKILL_HISTORY`), last 24 draws |
| `inspect.mjs` (+ `lib/cdp.mjs`, `lib/probe.mjs`, `lib/rules.mjs`) | loads the page under test and whatever it loads; serves a local file over a throwaway server on 127.0.0.1 (random port, root = current directory) | `.taste-skill/inspect/` (screenshots, `report.json`); a temporary Chrome profile under `$TMPDIR`, removed afterwards. Spawns headless Chrome. Reads `.taste-skill/direction.json`, or failing that the `direction.json` beside its `--out` folder, and skips every rule in its `allow` list |
| `assets.mjs` | search: api.openverse.org, commons.wikimedia.org, api.artic.edu, collectionapi.metmuseum.org, archive.org, images-api.nasa.gov; `--get` also downloads from the image host a result names | search writes nothing; `--get` writes the file and `credits.json` into `--to` (default `assets/`, the art director always passes `--to .taste-skill/<page>/assets`) |
| `lint.mjs` | none | none |

To update: read the new tree's scripts again, copy with `git archive`, replace the hashes above.

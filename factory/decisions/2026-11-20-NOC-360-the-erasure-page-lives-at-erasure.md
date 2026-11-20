# 2026-11-20 · NOC-360 · The erasure page lives at /docs/concepts/erasure/; /docs/concepts/shredding/ redirects to it

- Decided by: Carsten, 2026-11-20 on NOC-360 (Q5, relayed by pm)
- Why: the reader's name for the act is "erasure" (`docs/glossary.md`); "shred" is one step of it. Links to the old address may already exist outside the site, so the redirect in `site/astro.config.mjs` stays, and new links use the new address. `site/scripts/texts.test.mjs` checks both.
- Instead of: the page at `/docs/concepts/shredding/`.
- Topics: site, erasure

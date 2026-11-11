# site/

The NoCentralDB landing page and docs (NOC-211): Astro, with Starlight for the docs under `/docs` and `starlight-llms-txt` for `/llms.txt`. It is its own pnpm project, outside the repository's workspace.

## Build

```sh
pnpm --dir site install
pnpm --dir site build
```

The static site lands in `site/dist/`, which git ignores. `pnpm --dir site preview` serves it at http://localhost:4321.

## Screenshots

With the preview running, `pnpm --dir site shots` writes `drafts/magenta-desktop.png` (1440 px) and `drafts/magenta-mobile.png` (390 px), at deviceScaleFactor 2. `drafts/` is ignored too: screenshots go to the pull request, never into git.

## Brand

Colours, type, logo and copy follow the NoCentralDB brand in `nxsflow/brand`, commit 38abcfc: BRAND.md, section "NoCentralDB (endorsed)". `src/styles/tokens.css` copies `tokens/nocentraldb.css` unchanged. On top of it the page is 60/30/10: near-black, safety yellow #F2DC4B and hot magenta #FF3D8B (`--loud`), as `factory/decisions/2026-11-11-NOC-211-the-landing-page-is-60-30-10-with-hot-magenta.md` records.

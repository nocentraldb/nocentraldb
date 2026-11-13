# site/

The NoCentralDB landing page and docs (NOC-211): Astro, with Starlight for the docs under `/docs` and `starlight-llms-txt` for `/llms.txt`. It is its own pnpm project, outside the repository's workspace.

## Build

```sh
pnpm --dir site install
pnpm --dir site build
```

The static site lands in `site/dist/`, which git ignores. `pnpm --dir site preview` serves it at http://localhost:4321.

## Screenshots

With the preview running, `pnpm --dir site shots` writes `drafts/landing-desktop.png` (1440 px, full page), `drafts/landing-mobile.png` (390 px, full page) and `drafts/landing-desktop-670.png` (the first screen of a 1440x670 window), at deviceScaleFactor 2. `drafts/` is ignored too: screenshots go to the pull request, never into git.

## Brand

Colours, type, logo and copy follow the NoCentralDB brand in `nxsflow/brand`, commit 38abcfc: BRAND.md, section "NoCentralDB (endorsed)". `src/styles/tokens.css` copies `tokens/nocentraldb.css` unchanged. On top of it the landing page uses two colours: near-black for its grounds and safety yellow #F2DC4B as the one accent, with one section flooded yellow, the close. Hot magenta #FF3D8B (`--loud` in `src/styles/scene.css`) appears only inside the scenario's drawing, as the mark of a live connection. `factory/decisions/2026-11-12-no-ticket-the-landing-page-leads-with-speed-in-two-colours.md` records this; it supersedes the earlier 60/30/10 decision. The landing page is built on a vendored copy of the tasteskill design kit in `public/assets/` (MIT, see `public/assets/LICENSE-taste-skill.md`), which is never edited.

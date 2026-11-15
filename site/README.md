# site/

The NoCentralDB landing page and docs (NOC-211): Astro, with Starlight for the docs under `/docs` and `starlight-llms-txt` for `/llms.txt`. It is its own pnpm project, outside the repository's workspace.

## Build

```sh
pnpm --dir site install
pnpm --dir site build
```

The static site lands in `site/dist/`, which git ignores. `pnpm --dir site preview` serves it at http://localhost:4321.

## The live demo

The landing page's section "Try it in this browser" (`src/components/Demo.astro`, NOC-319) runs the example task list of `apps/tasks-ts` on the visitor's browser, with its database on OPFS and no relay. Its HTML is part of the page; the demo itself is not. An inline loader waits for the page's load event, for the section to come near the viewport and for the browser to be idle, checks for OPFS, Web Locks, BroadcastChannel, workers and WebAssembly, and only then imports `/demo/demo.js` (`public/demo/demo.js`, in git). That module brings the SDK, its worker and its WebAssembly core from `/demo/sdk/`, which `scripts/demo-assets.mjs` copies from the SDK's build output, with the example's lock:

```sh
node packages/sdk/build.mjs           # the SDK, its core compiled to WebAssembly
node site/scripts/demo-assets.mjs     # into site/public/demo, which git ignores but demo.js
pnpm --dir site build
```

Without those two steps the site builds as before and the section says it could not load the demo. The deploy (`site-pages.yml`) runs them and requires the demo's files. `moon run repo:site-demo` checks the demo in Chromium (`scripts/site-demo/`); `node scripts/site-demo/measure.mjs` measures the page's paints, with `--throttle` for a slow phone and `--dist` for another build.

## Screenshots

With the preview running, `pnpm --dir site shots` writes `drafts/landing-desktop.png` (1440 px, full page), `drafts/landing-mobile.png` (390 px, full page) and `drafts/landing-desktop-670.png` (the first screen of a 1440x670 window), at deviceScaleFactor 2. `drafts/` is ignored too: screenshots go to the pull request, never into git.

## Brand

Colours, type, logo and copy follow the NoCentralDB brand in `nxsflow/brand`, commit 74bf76f: BRAND.md, section "NoCentralDB (endorsed)". `src/styles/tokens.css` copies `tokens/nocentraldb.css`, formatted by Biome. On top of it the landing page uses two colours: near-black for its grounds and safety yellow #F2DC4B as the one accent, with one section flooded yellow, the close. Hot magenta #FF3D8B (the brand's role `loud`, `--ncdb-loud`) appears only inside the scenario's drawing, as the mark of a live connection. `factory/decisions/2026-11-12-no-ticket-the-landing-page-leads-with-speed-in-two-colours.md` records this; it supersedes the earlier 60/30/10 decision. The landing page is built on a vendored copy of the tasteskill design kit in `public/assets/` (MIT, see `public/assets/LICENSE-taste-skill.md`), which is never edited.

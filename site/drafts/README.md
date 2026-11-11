# Landing page draft (NOC-211)

Screenshots of the draft at desktop 1440 px and mobile 390 px, deviceScaleFactor 2: `magenta-desktop.png` and `magenta-mobile.png`. The built site is in `site/dist/`.

**Brand source:** `nxsflow/brand`, commit 38abcfc ("feat(nocentraldb): add NoCentralDB as an endorsed brand"): BRAND.md, section "NoCentralDB (endorsed)", and `tokens/nocentraldb.css`, which `site/src/styles/tokens.css` copies unchanged.

**Colour, 60/30/10 (Carsten, 2026-11-11):** about 60 % near-black (`bg` #111110 and the surfaces), about 30 % safety yellow #F2DC4B as grounds and panels, and about 10 % hot magenta #FF3D8B (`--loud`) for the primary action, the connected state, link underlines and focus. On nocentraldb.io this overrides BRAND.md's "accent under 5 %"; `factory/decisions/2026-11-11-NOC-211-the-landing-page-is-60-30-10-with-hot-magenta.md` records it.

Regenerate: `pnpm build`, `pnpm preview`, then `pnpm shots` in another shell.

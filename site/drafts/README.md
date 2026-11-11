# Landing page draft (NOC-211)

Screenshots of the draft at desktop 1440 px and mobile 390 px, deviceScaleFactor 2: `landing-desktop.png` and `landing-mobile.png`. The built site is in `site/dist/`.

The design follows the NoCentralDB brand in `nxsflow/brand`, commit 38abcfc ("feat(nocentraldb): add NoCentralDB as an endorsed brand"): BRAND.md, section "NoCentralDB (endorsed)", and `tokens/nocentraldb.css`, which `site/src/styles/tokens.css` copies unchanged. Dark only; one safety-yellow accent for the primary action, the connected state, link underlines and focus; the wordmark and favicons from `logos/nocentraldb/`.

Regenerate: `pnpm build`, `pnpm preview`, then `pnpm shots` in another shell.

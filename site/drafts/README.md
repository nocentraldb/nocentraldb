# Landing page drafts (NOC-211)

Screenshots of the draft at desktop 1440 px and mobile 390 px, deviceScaleFactor 2, one pair per candidate for the loud third colour: `<colour>-desktop.png` and `<colour>-mobile.png`. The built site is in `site/dist/`: `index.html` shows signal blue, and `accent/<colour>/index.html` shows each candidate, with a switcher at the bottom of the page.

**Brand source:** `nxsflow/brand`, commit 38abcfc ("feat(nocentraldb): add NoCentralDB as an endorsed brand"): BRAND.md, section "NoCentralDB (endorsed)", and `tokens/nocentraldb.css`, which `site/src/styles/tokens.css` copies unchanged.

**Colour, the owner's 60/30/10 rule on top of the brand (2026-11-11):** about 60 % near-black (`bg` #111110 and the surfaces), about 30 % safety yellow #F2DC4B as grounds and panels, and about 10 % a loud third colour for the primary action, the connected state, link underlines and focus. The owner picks the loud colour:

| Candidate | Hex | Text on it | Why |
| --- | --- | --- | --- |
| Signal blue | `#2F5BFF` | ink #ECEBE4 | Yellow's near-complement, so the two never blur. On a marked floor, blue means a mandatory action, which is what it marks here. |
| Safety green | `#1FD38A` | bg #111110 | The safe-condition colour of the same floor; 9.7:1 on the ground, so it carries the connected state well. Next to yellow it is the softest contrast of the three. |
| Hot magenta | `#FF3D8B` | bg #111110 | The loudest against both black and yellow, and no safety meaning to misread. |

Left out: violet (the typical AI-made page), and red, orange or any second yellow, which would read as a warning.

Regenerate: `pnpm build`, `pnpm preview`, then `pnpm shots` in another shell.

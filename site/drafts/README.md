# Landing page drafts (NOC-211)

Screenshots of the first draft, at desktop 1440 px and mobile 390 px, deviceScaleFactor 2, one pair per candidate accent colour. The built site is in `site/dist/`: `index.html` shows magenta, and `accent/<name>/index.html` shows each candidate with a switcher at the bottom of the page.

The palette follows 60/30/10: near-black `#121211` (60 %), the brand's lime `#B9FF66` (30 %), and one loud accent (10 %) for the main buttons, the selected tab, the relay's byte counts and focus rings. The owner picks the accent.

| Candidate | Hex | Text on it | Why |
| --- | --- | --- | --- |
| Signal magenta | `#FF2E7E` | near-black | The loudest contrast to lime and black; warm, and no other nxsflow brand uses it. |
| Electric cobalt | `#2B59FF` | off-white | A cool, technical counterweight to lime; it reads as trust and the network. |
| Wire cyan | `#00C8FF` | near-black | Bright on black and the most "live" of the three; the risk is that it sits close to lime in energy. |

Violet was left out on purpose: it is the colour of the typical AI-made page.

Regenerate: `pnpm build`, `pnpm preview`, then `pnpm shots` in another shell.

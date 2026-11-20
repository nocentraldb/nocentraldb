# 2026-11-20 · NOC-360 · The landing page calls the relay "the server in the middle" or "a server in the cloud"; RFCs and docs say relay

- Decided by: Carsten, 2026-11-12 ("The landing page leads with speed, in two colours") and 2026-11-20 on NOC-360 (Q1 and Q6, relayed by pm)
- Why: the landing page's hero and scenario explain the mechanism in plain words for a first-time visitor. There the relay is "a server in the cloud", drawn as "Cloud server", and the hero's status line says "the server in the middle". From the landing page's code section on, and in every other text (the docs, the RFCs, the SDKs), it is "relay", as `docs/glossary.md` defines it. `site/scripts/landing.test.mjs` holds that the hero and scenario never say "relay".
- Instead of: "the relay" in the hero's status line, as pm first recommended for Q1.
- Topics: site, texts, glossary

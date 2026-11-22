# 2026-11-22 · NOC-468 · Another member's rich-text update reaches Yrs only through the core's own format check and a second guard, and the fold stays off in published builds until both are in place

- Decided by: Carsten, 2026-11-22, on NOC-468, through pm
- Why: a rich-text update is bytes another member wrote. The core reads an update's encoding itself and parks one that breaks the format before Yrs sees it, with a second guard (NOC-468) beside that check. Until both are in place, the fold sits behind the cargo feature `rich-text`, which is off in every published build and package; there, rich text parks as an unknown type.
- Instead of: isolating the fold in another process or wasm instance, which stays the fallback.
- Topics: rich-text, rfc-0013, yjs, robustness

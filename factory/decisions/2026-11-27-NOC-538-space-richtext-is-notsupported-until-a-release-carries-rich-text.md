# 2026-11-27 · NOC-538 · space.richText exists in the SDK and answers notSupported until a release carries rich text

- Decided by: Carsten, 2026-11-27, on NOC-538 (Q5), through pm
- Why: the fold stays off in every published build until its guard is complete (NOC-468); an app can be written against the call now, and finds out at once, with a code of its own, that this build does not hold rich text yet.
- Instead of: leaving the call out of the SDK's public types until a release carries rich text.
- Topics: rich-text, rfc-0013, rfc-0009, sdk-ts, errors

# 2026-11-16 · NOC-343 · An invalid lock's refusals follow error.lockInvalid's colon, one line after another, unindented

- Decided by: coding agent (NOC-343)
- Why: `Invalid`'s Display prints the refusals' lines as `Refusal`'s own Display does, joined by a newline, so error.lockInvalid stays one text with `{invalid}` and a single refusal reads as one sentence after the colon; an index whose lookup would not run through it is modelRefused with core.viewPlan as the whole message, as core.viewRefused is
- Instead of: indenting each line under a header, as core.viewRefused and `ncdb model build` do, which would need error.lockInvalid's text to end in a bare colon
- Topics: texts, errors, model

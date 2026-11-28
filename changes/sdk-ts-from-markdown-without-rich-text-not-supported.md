---
type: fix
tickets: [NOC-595, NOC-590]
introduced: cycle
components: [sdk-ts]
---
## Symptom

In TypeScript, a create or an update that carried `nc.richText.fromMarkdown(...)` on a core built without rich text was refused with `invalidValue`, while `space.richText` on the same core answers `notSupported`.

## When

On every published build, none of which holds rich text yet, whenever a write carries a first document from Markdown.

## Cause

The SDK sent the first document to the core whether or not the core holds rich text, and a core without it refuses the value as one it cannot write.

## Fix

The SDK refuses such a write with `notSupported` before it reaches the core, as `space.richText` does, so an app learns from either call that this build does not hold rich text (RFC 0013, "An editor: the binding").

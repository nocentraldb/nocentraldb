# 2026-11-09 · NOC-80 · nc.Infer types a link as a string id, and types only the fields

- Decided by: coding agent (NOC-80)
- Why: no RFC fixes how a 16-byte object id looks in TypeScript, and RFC 0009's examples pass `taskId` and `taskA.$id` as plain values, so a link reads as a `string` and many links as `string[]`, through an unexported alias `ObjectId` that adds no public name. A `string` needs an encoding of the 16 bytes, which no RFC names; this PR chooses none, since nothing builds or reads an id yet, and puts the shape and its encoding to the owner on PR #91. `nc.Infer` types the fields as RFC 0005's "Types for the app" lists them; the `$id`, `$space`, `$deleted`, `$conflicts` and `$raw` that RFC 0009 puts on a read object belong to the ticket that builds reads. `nc.Unknown` is `{ unknown: string }`, since a choice holds a text. Reserved field types (`nc.richText`, `nc.stream`) read as `never`.
- Instead of: a `Uint8Array` of the 16 bytes, or a branded string type, each a public shape no RFC names; putting the `$` keys into `nc.Infer` before reads exist.
- Topics: sdk, api, types, rfc-0005, rfc-0009

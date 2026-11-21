# 2026-11-21 · NOC-428 · The TypeScript SDK erases a person with the Rust calls in camel case

- Decided by: Carsten, 2026-11-21, on NOC-428 (A, "Passt wie empfohlen")
- Why: RFC 0009 named only the Rust calls for erasing a person (NOC-326), and #424 built them in TypeScript (NOC-406). The names mirror Rust in camel case and follow NOC-321's pattern for pairing: `db.erasureRequest(object)` takes one of the person's objects as the SDK read it, so the person id never reaches JavaScript; `ErasureRequest` and `SignOffs` are their texts (`ncdb-erase-1:…`, `ncdb-sign-off-1:…`), read back by `nc.erasureRequest(text)` and `nc.signOffs(text)`, which check only the prefix and the encoding; `db.account.signOff(request)` gives `{ signOffs, notSigned }`; `db.erase(request, signOffs)` gives `{ shreds, skipped, notice }`, objects where Rust has tuples, as for pairing; `space.erasures.takeBack(shred)` gives the notice `takenBack`. The errors are the Rust SDK's: `noPerson`, `notAllowed`, `invalidValue`. RFC 0009's "Erasing a person" says so.
- Instead of: other names or shapes (option B), changed before the next beta.
- Topics: sdk-ts, erasure, public-names

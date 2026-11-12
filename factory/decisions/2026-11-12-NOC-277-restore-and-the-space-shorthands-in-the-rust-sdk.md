# 2026-11-12 · NOC-277 · The Rust SDK restores a deleted object, and a space has RFC 0009's four one-call shorthands

- Decided by: pm, NOC-277 (option A: all four shorthands with `Transaction::restore`), on RFC 0009's own names; the clear of a field waits for the owner on NOC-277
- Why: RFC 0009, "One write, one change", names `tx.restore(collection, id)`, delete (4) with `false`, and `project.create`, `project.update`, `project.delete` and `project.restore` as shorthands for a write with one call. The Rust SDK had neither, and NOC-176's TypeScript SDK needs restore. The transaction's delete record now carries its boolean; `restore` writes `false` and is refused with `NotFound` when the copy does not hold the object, as an update or a delete is (RFC 0009, "What the SDK refuses"). Each shorthand is `write` with its one call, so one change, and answers what `write` answers, `Written`, so `create`'s new id is its only id.
- Instead of: only `Space::restore`, which would have been the one shorthand of four an app reaches for last; a shorthand of `create` that answers the id alone, which would answer less than `write` does and hide the change id.
- Topics: sdk-rust, write, api

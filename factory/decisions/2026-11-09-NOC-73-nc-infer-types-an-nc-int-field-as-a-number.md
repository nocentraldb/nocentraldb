# 2026-11-09 · NOC-73 · nc.Infer types an nc.int field as a number: writes must be safe integers, larger values read undefined

- Decided by: Carsten (RFC 0005 decision D16)
- Why: ints in apps are priorities, counts, sizes and positions, far below 2^53, and RFC 0009's examples already write them as `number`. A write outside −(2^53 − 1) to 2^53 − 1 is refused with `invalidValue`; a stored value beyond that range, which another member or the Rust SDK can write, reads `undefined` and is named in `$raw`, never silently rounded (principle 4), and `space.inspect` returns it exactly as a `bigint`. A variant typed `bigint` can come later as a new field function without changing the lock.
- Instead of: `bigint`, where every literal needs `3n`, `JSON.stringify` throws and mixing with a `number` throws a `TypeError`; `number | bigint`, which makes every use of every int field handle both types.
- Topics: rfc-0005, rfc-0009, sdk, types

# 2026-11-20 · NOC-362 · Erasure is a work order; 0.1's Db::erase is an interim call the work-order RFC replaces

- Decided by: Carsten, 2026-11-20, on NOC-362 ("Ja zu den Empfehlungen")
- Why: the design keeps erasure as a work order that one member performs and a second account signs off (RFC 0001, RFC 0003). Until the work-order RFC exists, the Rust SDK erases with `Db::erase` (NOC-326); RFC 0001, RFC 0003 and RFC 0009 call it an interim call that the work-order RFC will replace.
- Instead of: making `Db::erase` the lasting design of erasure.
- Topics: erasure, work-orders, sdk-rust

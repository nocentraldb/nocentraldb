# 2026-11-12 · NOC-232 · Plugins depend on the public crate ncdb-plugin and are checked by the public crate ncdb-harness

- Decided by: Carsten, 2026-11-12, on NOC-232 ("Ich stimme den Vorschlägen zu"), RFC 0011 decisions 6 and 7
- Why: `ncdb-plugin` holds only what a plugin needs (`FieldType`, `Data`, `Origin` and its ids, three park reasons, `Plugin`), so the core stays free to change its view, keys and bytes. The convergence harness leaves `ncdb-core`'s tests as `ncdb-harness`, the built-in types move onto it, and a plugin runs C1 to C7 and the `decode_state` round trip in its own CI, natively and on wasm32; the core cannot prove a third party's plugin converges.
- Instead of: a public module inside `ncdb-core`, which ties every plugin to the core's version; an `ncdb plugin check` command, which can wrap the crate later; a short harness run on every registration, which slows every open and proves little.
- Topics: rfc-0011, plugins, crates, testing, convergence

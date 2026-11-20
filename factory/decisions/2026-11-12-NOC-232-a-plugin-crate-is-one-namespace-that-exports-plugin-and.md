# 2026-11-12 · NOC-232 · A plugin crate is one namespace that exports plugin(), and a Rust app hands it over with OpenOptions::plugin

- Decided by: Carsten, 2026-11-12, on NOC-232 ("Ich stimme den Vorschlägen zu"), RFC 0011 decisions 2 and 3
- Why: `plugin()` returns a `Plugin` with its namespace, types and rule versions, and the registry maps a type id and rule to its code, refusing two plugins for one namespace or one for `ncdb`; the app calls `OpenOptions::plugin` once per plugin, so plugins are Rust compiled into the app's core, as RFC 0004 decided, with no loading at run time.
- Instead of: registering each type on its own, which ties no namespace to one crate; registration at link time through `inventory` or `linkme`, implicit and unreliable on wasm32; one `OpenOptions::plugins(...)` set, one more type for no gain.
- Topics: rfc-0011, plugins, sdk-rust, api

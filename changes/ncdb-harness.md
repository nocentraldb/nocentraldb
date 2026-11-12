---
type: feature
tickets: [NOC-249]
components: [core]
---
## Problem
RFC 0004 accepts a plugin field type only when it passes the convergence harness, C1 to C7. The harness was test code inside `ncdb-core`, which a plugin's crate could not use.

## Use
A plugin's crate adds the new crate `ncdb-harness` as a dev-dependency. It wraps its field type in `PluginState`, writes a `Generator` of its operations, usually through `PluginState::write` as an app writes, and calls `check`, natively and on wasm32, with broken variants of the type that `check` must reject (RFC 0011, "Checking a plugin: ncdb-harness"). The harness also checks that `decode_state` round-trips, and reads an operation the type parks as `Outcome::Parked`. The core's built-in types are checked by the same crate.

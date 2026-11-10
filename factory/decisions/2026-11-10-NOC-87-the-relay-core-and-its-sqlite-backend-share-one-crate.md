# 2026-11-10 · NOC-87 · The relay core and its SQLite backend share one crate, `ncdb-relay`

- Decided by: coding agent (NOC-87)
- Why: RFC 0007 puts every check in one relay core that every deployment links, and the SQLite backend is the only backend yet. One native-only crate holds the checks (`Relay`), the backend interface (`store::RelayStore`), the one conformance suite as a public module (`conformance`, with `conformance_suite!` that writes one test per rule for any backend) and the SQLite backend (`sqlite`). The suite is written against the interface alone, so the Postgres and DynamoDB backends call it from their own crates. The interface's methods return `impl Future + Send`, the native form of RFC 0007's `async fn`, so a server can run them on any thread, without the `async_trait` crate; the suite and the tests poll them with a small `block_on` and need no runtime.
- Instead of: a separate `ncdb-relay-sqlite` crate, which would make the core's tests depend on a crate that depends on the core; or the `async_trait` attribute RFC 0007 draws, which adds a dependency and boxes every call.
- Topics: relay, rfc-0007, sqlite, crates

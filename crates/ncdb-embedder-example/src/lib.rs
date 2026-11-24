//! An example of an app's own tests against a real relay (NOC-507). The
//! crate holds no code: its tests, in `tests/`, use only what an app that
//! embeds the Rust SDK adds to its own dev-dependencies, the SDK
//! `nocentraldb` and the test relay of `ncdb-relay-server`
//! (`ncdb_relay_server::local::TestRelay`).

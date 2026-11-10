---
type: internal
tickets: [NOC-108]
components: [sdk-rust, core]
---
The Rust SDK `nocentraldb`, not yet released, keeps a database at `path(dir)` with its member key and account key in the platform keystore or a `key_file(path)`, finds its keys and spaces again on reopen, certifies an `agent()` as an agent, and refuses opening with neither a path nor `in_memory()` (`InvalidValue`) or with neither a keystore nor a key file (`KeyStoreUnavailable`), as RFC 0009 S19, S20, S26 and S27 decide.

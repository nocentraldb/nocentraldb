---
type: internal
tickets: [NOC-88]
components: [sdk-rust, core]
---
The Rust SDK `nocentraldb`, not yet released, opens an in-memory database from a lock, creates a space, writes one sealed change per `space.write`, reads by id, queries declared indexes and subscribes to a query's page, locally, as RFC 0009 names these calls.

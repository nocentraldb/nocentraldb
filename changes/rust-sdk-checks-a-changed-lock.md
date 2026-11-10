---
type: internal
tickets: [NOC-148]
components: [sdk-rust, core]
---
The Rust SDK `nocentraldb`, not yet released, keeps the lock a database was last opened with and checks a changed lock against it on open (RFC 0005, D11): an additive change opens the database and folds what the old model parked, an older lock opens it as an old client, and another `model.id` or a change that is not additive is `ModelRefused`, naming each refused change.

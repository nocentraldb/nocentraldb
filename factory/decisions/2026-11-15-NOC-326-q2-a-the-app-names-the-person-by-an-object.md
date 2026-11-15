# 2026-11-15 · NOC-326 · The app names the person to erase by one of the person's objects

- Decided by: Carsten, 2026-11-15, on NOC-326 (Q2 a)
- Why: the person id lives sealed in the hidden `$person` field and is never shown to the app (RFC 0005, "Identifying the person"). `db.erasure_request(&object)` reads it from the object, as `Create { same_person_as }` does, so the person id stays inside the SDK.
- Instead of: a public `PersonId` type, new API surface whose value could end up in an app's logs.
- Topics: sdk-rust, erasure, personal-fields

# 2026-11-16 · NOC-354 · A reference in Rust is `Reference { space, id }`, written with `Value::from` and read with `get::<Reference>`

- Decided by: Carsten (owner), 2026-11-16, NOC-354 comment ("Option 1 passt."), on pm's recommendation
- Why: RFC 0009 gave no Rust form for an `nc.ref` value. `nocentraldb::Reference { space: SpaceId, id: ObjectId }`, `#[non_exhaustive]` and made with `Reference::new`, follows `Data`: one type to write and to read. It reads clearly and spells out the field type's name. RFC 0009's Rust section and its "Public names" table carry it.
- Instead of: a pair, `Value::from((SpaceId, ObjectId))` and `get::<(SpaceId, ObjectId)>`, which needs no new name but reads less clearly; and `Ref`, which abbreviates.
- Topics: sdk, rust, references, public-names

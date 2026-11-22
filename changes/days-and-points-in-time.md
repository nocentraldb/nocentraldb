---
type: breaking
tickets: [NOC-412]
components: [core, sdk-rust, sdk-ts, cli]
---
## Problem

An app could keep a date only as text, such as `"2026-11-21"` in an `nc.string()` field. Nothing checked the text, so `"21.11.2026"` or `"2026-02-30"` could sit in the same field, and an index sorted the dates by their characters, not in time order.

## Use

Two new field types. `nc.date()` holds a calendar day, such as a birthday: it has no time and no time zone, and reads as the same day everywhere. `nc.timestamp()` holds a point in time to the millisecond, in UTC: one moment, which each reader shows in its own time zone. Both merge as basic values, the last writer wins, and both can be indexed and made personal.

```ts
export const model = nc.model({
  people: nc.collection({
    birthday: nc.date().personal({ person: "self" }),
  }),
  events: nc.collection(
    { occurredAt: nc.timestamp() },
    { indexes: { latest: nc.index([{ field: "occurredAt", direction: "desc" }]) } },
  ),
});

await space.create(model.people, { birthday: "1990-04-02" });
await space.create(model.events, { occurredAt: new Date() });
```

In TypeScript a day is a `"YYYY-MM-DD"` string and a point in time a `Date`. In Rust they are `nocentraldb::Date` and `nocentraldb::Timestamp`. Both types cover the years 1 to 9999, and the SDKs refuse anything else with `invalidValue`. A field that holds dates as text keeps its type: add a new `nc.date()` field, write the values into it, and deprecate the text field. An app that needs the time zone a person meant, for example for a weekly meeting at 9:00 in Berlin, stores the zone's name in a field of its own.

## Migration

Apps need no change: nothing that worked before changes meaning, and an existing field keeps its type.

Rust code that matches on `ncdb-core`'s field kinds needs two new arms. Both `ncdb_core::model_lock::Kind` and `ncdb_core::basic::Kind` gain the variants `Date` and `Timestamp`, and neither enum is `#[non_exhaustive]`, so an exhaustive `match` on either stops compiling until it handles them or gets a wildcard arm. The SDKs' own types are unchanged.

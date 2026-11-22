---
type: internal
tickets: [NOC-414]
components: [sdk-rust]
---
A guide, docs/bulk-import.md, shows how to write a burst of operations into a space in batches, from an import or from a device coming back after working offline, with numbers measured at 81,000 operations through the local relay; the example `bulk_import` of `nocentraldb` reproduces them, and a test runs both cases at 2,000 operations.

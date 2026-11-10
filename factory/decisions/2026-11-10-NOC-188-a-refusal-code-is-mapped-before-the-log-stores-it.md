# 2026-11-10 · NOC-188 · A relay's refusal code is mapped in the SDK before the log stores it, and stored codes are not mapped on read

- Decided by: coding agent (NOC-188), following the owner's answer A on NOC-146
- Why: the SDK maps the code where it receives a push result (`apply::pushed`) with the same function as `SyncStatus::error`, `wire::mapped_code`, before `View::refuse` stores it; so ncdb-core's view stays unchanged, holds no list of RFC 0007's codes, and the on-disk format does not change. A code stored before this change is not mapped on read, and needs no migration, because nothing reads it: the SDK only counts refused changes (`View::relay_counts`), and no API shows a refusal's code to the app.
- Instead of: mapping in `View::refuse` or `View::refused` in ncdb-core, which would put RFC 0007's code list into the core; or an SDK reader that maps stored codes now, which no caller would use.
- What this does not do: a database written before this change may still hold a relay's own text in the `relay_refusal` column. The API that first shows a refusal's code to the app (RFC 0009's refused changes, not built yet) must read it through `wire::mapped_code`.
- Topics: sdk-rust, rfc-0007, errors

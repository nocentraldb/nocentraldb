# 2026-11-12 · NOC-274 · The TypeScript SDK in the browser: its names, its worker and its package

- Decided by: Carsten, 2026-11-12, on NOC-274 (A for points 1 to 6; point 7 changed)
- Why: NOC-176 builds the TypeScript SDK in the browser on the Rust SDK compiled to wasm, and RFC 0009 left these points open:
  1. The bridge between the worker and the Rust SDK is a new crate, `nocentraldb-wasm`, `publish = false`, so `nocentraldb` keeps its browser calls crate-internal.
  2. `nc.open`'s option that names the browser database is `name`, `"default"` when left out.
  3. `@nocentraldb/sdk` ships the worker script and the wasm module, finds them with `new URL(..., import.meta.url)`, and gains a build step with a `dist/` output.
  4. `nc.Error` has `code`, `ids` and `fingerprint`.
  5. The exported types are `Db`, `Space`, `Tx`, `Invitation`, `Page<T>`, `Query<T>`, `Notice`, `SyncStatus`, `SpaceStatus` and `Durability`; a fingerprint is a plain string of twelve blocks.
  6. `take` is at most 1000; a larger value is `invalidValue`, in both SDKs.
  7. Changed from the proposal: `@nocentraldb/sdk` becomes publishable with NOC-176's first slice, and its alphas are published to npm under the dist-tag `alpha`. The alpha job and the owner's first manual publish are the follow-up NOC-281; docs/release-management.md already describes the alpha dist-tag, so RFC 0009 records only what concerns the SDK's API.
- RFC 0009 records points 2 to 6 in "Open and close", "Queries and indexes", "Errors" and "Public names".
- Instead of: a `nocentraldb::wasm` module, which would make `OpenOptions::browser` a public Rust name; an option that names the worker's URL; no upper bound on `take`; a fingerprint object in TypeScript; publishing only with the first beta.
- Topics: sdk-ts, browser, api, wasm, release

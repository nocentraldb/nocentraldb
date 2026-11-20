---
type: internal
tickets: [NOC-371]
components: [sdk-ts]
---
The TypeScript code of the landing page's "Get started" section is type-checked against the SDK in the gates, as its Rust code is compiled, so it cannot drift from `@nocentraldb/sdk`.

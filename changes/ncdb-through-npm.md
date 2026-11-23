---
type: feature
tickets: [NOC-415, NOC-443]
components: [cli, sdk-ts]
---
## Problem
`ncdb` came only as a Rust crate, so a JavaScript app and its CI needed a Rust toolchain just to build `model.lock.json`.

## Use
`@nocentraldb/sdk` now carries `ncdb` as a bin. In the app's folder, with Node.js 24 or later and the app's `package.json` set to `"type": "module"`:

```sh
npm install @nocentraldb/sdk@alpha
npx ncdb model build        # reads ./model.ts, writes ./model.lock.json
npx ncdb model check        # fails if the lock is missing or out of date, or the change is refused
```

It runs the same code as `cargo install ncdb`, compiled to WebAssembly, so it takes the same options and writes and says the same.

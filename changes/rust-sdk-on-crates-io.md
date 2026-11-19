---
type: feature
tickets: [NOC-372]
components: [sdk-rust, cli, core]
---
## Problem
Rust users had to depend on the repository's git tag to use the Rust SDK or the `ncdb` CLI, because no crate went to crates.io.

## Use
From this beta on, the Rust SDK and the CLI are on crates.io, with the core crates they need, `ncdb-core` and `ncdb-plugin`:

```sh
cargo add nocentraldb@0.1.0-beta.1
cargo install ncdb@0.1.0-beta.1
```

Alphas stay off crates.io. To use an alpha, depend on its git tag.

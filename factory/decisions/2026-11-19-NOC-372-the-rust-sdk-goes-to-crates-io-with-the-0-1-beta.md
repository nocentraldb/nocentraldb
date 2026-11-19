# 2026-11-19 · NOC-372 · The Rust SDK goes to crates.io with the 0.1 beta

- Decided by: Carsten, 2026-11-19, in chat with pm
- Why: Rust users install the beta with `cargo add nocentraldb@0.1.0-beta.1` instead of depending on a git tag. `nocentraldb` and `ncdb` drop `publish = false`, and with them the workspace crates they depend on at runtime, `ncdb-core` and `ncdb-plugin`, because a crate on crates.io can only depend on crates on crates.io. The relay, AWS, Lambda, harness, test and wasm crates stay unpublished: no published crate needs them at runtime (docs/release-management.md, "Published packages"). Alphas still never go to crates.io. The owner configures crates.io Trusted Publishing once for the four crates, whose `0.0.0` placeholders the owner already holds.
- Instead of: keeping Rust on the git tag through 0.1, with `cargo add nocentraldb --git … --tag v0.1.0-beta.1` on the website (option B).
- Topics: release, crates-io, sdk-rust, cli

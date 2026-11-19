# 2026-11-19 · NOC-372 · Each published crate carries the license texts as symbolic links to the root files

- Decided by: coding agent (NOC-372), for the owner's review on the PR
- Why: the MIT license asks that its text travel with every copy, and a crate on crates.io is a copy. The SPDX field `license` packages no file, and cargo packages only files inside the crate's directory, apart from `readme` and `license-file`. `crates/<crate>/LICENSE-MIT` and `LICENSE-APACHE` link to the root files, and cargo packages the links as files (checked with `cargo publish --dry-run`). One text stays in one place.
- Instead of: copies of the license files in each crate, which can drift from the root; or no license text in the package.
- Topics: release, crates-io, licensing

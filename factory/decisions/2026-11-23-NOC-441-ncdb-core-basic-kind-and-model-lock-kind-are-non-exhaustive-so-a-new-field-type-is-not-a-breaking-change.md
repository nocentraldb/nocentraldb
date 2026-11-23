# 2026-11-23 · NOC-441 · ncdb-core's basic::Kind and model_lock::Kind are #[non_exhaustive], so a new field type is not a breaking change of the crate

- Decided by: Carsten, 2026-11-23, on NOC-441
- Why: every new field type (dates, rich text, later files and streams) added a variant and made ncdb-core's release breaking; one last breaking change, shipped in 0.2.0 with the dates, keeps the Rust API additive as GENESIS asks of schema changes.
- Instead of: leaving both enums exhaustive, so that matching code sees a new kind at compile time but every field type breaks ncdb-core.
- Topics: core, semver, field-types, rust-api

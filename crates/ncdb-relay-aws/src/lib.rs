//! The relay's AWS backend (RFC 0007, "The AWS backend"). Empty for now:
//! the crate holds the check of what the AWS Emulator covers of the
//! operations the backend needs (NOC-275), in `tests/emulator_coverage.rs`,
//! before any backend code is built on it. It is the one crate that links
//! the AWS SDK, as a dev-dependency so far.

//! The AWS backend passes the one backend test suite of RFC 0007 ("The
//! backend interface"), against the AWS Emulator. The tests are ignored
//! unless an emulator runs on this machine (docs/aws-emulator.md); the CI
//! job `aws-emulator` runs them.

#[path = "backend/fresh.rs"]
mod fresh;

use fresh::fresh;

ncdb_relay::conformance_suite!(
    fresh,
    ignore = "needs the AWS Emulator on this machine, with telemetry off: see docs/aws-emulator.md"
);

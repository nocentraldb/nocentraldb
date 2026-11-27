# 2026-11-27 · NOC-544 · cargo-deny reports unmaintained dependencies, and each exception names its ticket and expiry

- Decided by: coding agent (NOC-544), on pm's plan
- Why: without a `deny.toml`, cargo-deny's default `unmaintained = "workspace"` reports only workspace crates, so the zbus chain of NOC-408 and `smallstr` through `yrs` stayed hidden while embedders that check all crates went red. The root `deny.toml` sets `unmaintained = "all"`. An advisory that cannot be fixed yet goes into `ignore` with a `reason` that names its ticket and an expiry date, plus a comment that says what removes it. On the expiry date the exception is removed or renewed on its ticket. cargo-deny has no expiry field, so the date lives in the text.
- Instead of: keeping cargo-deny's defaults; ignoring an advisory without a ticket or date; setting `unmaintained = "workspace"` and checking dependencies by hand.
- Topics: dependencies, security-advisories, cargo

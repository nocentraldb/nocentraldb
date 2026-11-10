# 2026-11-10 · NOC-179 · An unlisted relay code shows in the Rust `SyncStatus::error` as `unknown_error`

- Decided by: coding agent (NOC-179), following the owner's answer A on NOC-146
- Why: the owner chose one fixed value, named `unknownError`, for every code RFC 0007 does not list. `SyncStatus::error` in Rust holds RFC 0007's codes as the protocol writes them, in snake case (`unsupported_version`, `removed_member`), and RFC 0009 gives every Rust name in snake case, so the Rust spelling is `unknown_error`. The SDK compares the relay's text, byte for byte, with the 19 codes of RFC 0007's table "The errors" where the relay's code enters the status (`Session::failed`); anything else, an empty, overlong or differently spelled code included, becomes `unknown_error`. A test reads the codes from RFC 0007's Markdown table, so a code the RFC adds fails the test until the SDK lists it too.
- Instead of: `unknownError` in camel case, which no other value of this field uses; or a length or character check, which would still pass a short text of the relay's own.
- What this does not do: a code the relay sends in a push result is recorded per change in the view's log as before, and is not mapped; `SyncStatus::error` does not show it.
- Topics: sdk-rust, rfc-0007, rfc-0009, errors

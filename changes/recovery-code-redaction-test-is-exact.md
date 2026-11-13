---
type: internal
tickets: [NOC-299]
components: [sdk-rust]
---
The recovery code's redaction tests compare with the exact `RecoveryCode(redacted)`, so a code with a word such as "act" or "code" no longer fails them.

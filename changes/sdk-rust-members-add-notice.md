---
type: internal
tickets: [NOC-167]
components: [sdk-rust]
---
The Rust SDK's `members().add` returns a `Notice` (`NoticeCode::MemberAdded`), refuses a certificate that does not hold with `InvalidCertificate` and a member added twice with `AlreadyMember`, `Error::fingerprint()` gives the account's fingerprint on `UnknownAccount`, and a `Fingerprint` shows as 60 digits in twelve blocks of five.

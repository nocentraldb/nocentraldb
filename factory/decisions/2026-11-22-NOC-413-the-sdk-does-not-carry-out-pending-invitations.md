# 2026-11-22 · NOC-413 · The SDK does not carry out pending invitations while the inviting admin's devices are off

- Decided by: Carsten, 2026-11-22, on RFC 0015's questions, through pm (NOC-429)
- Why: only a member that holds the space key can write an add, so no server may; the app's backend notifies the admin's devices, and an app may run an admin agent of the inviter's account in the cloud, as a visible member, on its own.
- Instead of: an admin agent that the SDK runs for every app.
- Topics: sdk-ts, sdk-rust, invitations, rfc-0015

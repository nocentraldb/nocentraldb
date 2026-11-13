---
type: feature
tickets: [NOC-268, NOC-236]
components: [sdk-rust]
---
`db.account().remove_member(member, &words)` answers a lost device or agent, as RFC 0010's "Losing a device" says. On another device of the account, with the recovery code, it:

- removes the lost member from every space the device administers and from the account space, rotating each space key;
- lists the spaces where no device of the account is an admin, so the app can ask their admins;
- draws a new account key with the next number and hands it to each remaining device through the account space, sealed to that device.

It returns a `Removed`: the spaces it removed the member from, each with its `MemberRemoved` notice; the spaces to ask admins about; and the notice `AccountKeyRenewed`.

The other devices take the new key when they sync, so their certificates are under it from then on. They also remove the lost member from the spaces they administer that the first device is not in. Every device carries the new statement into its spaces, and from then on an add certified by the old account key is refused with `InvalidCertificate`. A recovery code that is not the account's removes nothing and is refused with `notAllowed`.

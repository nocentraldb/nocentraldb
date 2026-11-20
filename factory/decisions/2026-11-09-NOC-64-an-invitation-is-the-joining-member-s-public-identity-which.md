# 2026-11-09 · NOC-64 · An invitation is the joining member's public identity, which the app carries to an admin in one hop

- Decided by: Carsten, 2026-11-09, in chat with pm, on #78 (RFC 0009 decision S12)
- Why: It is what RFC 0003 and use case 3 show, `members.add(invitation, { role })`, and one channel is enough, since nothing in an invitation is secret and no key bytes reach the app.
- Instead of: two hops, with the admin creating an invitation that the joining member accepts and answers, two trips through the app's channel (S12 B); the relay carrying invitations in a mailbox, which changes RFC 0007 and lets the relay see who is about to join which space (S12 C).
- Topics: rfc-0009, sdk, invitations, membership

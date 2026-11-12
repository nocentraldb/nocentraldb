# 2026-11-12 · NOC-266 · A device creates the account space with the first app space it creates, not when it opens

- Decided by: coding agent (NOC-266), checked by pm
- Why: RFC 0010 has the account's first device create the account space, and leaves when open. A relay lets only an account it admits create spaces (NOC-257), so an account space created at open is refused, for good, on a relay that admits the account only later. The first app space the device creates shows that the relay admits its account, so the account space comes with it. A fresh database then holds no change before the app writes one. A device that joins an account another device created (pairing, NOC-267) creates none.
- Gap it leaves: an account that only joins other accounts' spaces has no account space. On a relay that admits only listed accounts, that account's account-space genesis would also be refused later, when it pairs a second device. Whether the relay admits an account-space genesis from an account that is already a member of an admitted space is the owner's call; pm files it as a ticket.
- Instead of: creating the account space when the database opens, which a relay that admits listed accounts refuses for every account it does not list.
- Topics: accounts, sdk-rust, relay

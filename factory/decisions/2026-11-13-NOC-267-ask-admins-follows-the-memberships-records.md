# 2026-11-13 · NOC-267 · Which spaces pairing lists to ask their admins

- Decided by: pm, NOC-267
- Why: RFC 0010 lists the spaces "where no device of the account is an admin", and the account space's `memberships` records carry each device's role, so a device can tell that another device of the account administers a space it does not hold. `Paired::ask_admins` lists the spaces where neither this device nor, by those records, another device of the account is an admin; a space another device administers is in neither list, the rule NOC-268 uses too. A space with no record of an admin falls back to being listed.
- Instead of: listing every space of the account's other devices that this device does not hold, which would ask admins outside the account for spaces the account already administers.
- Topics: sdk-rust, accounts, pairing

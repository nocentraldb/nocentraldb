---
type: internal
tickets: [NOC-168]
components: [sdk-rust, core]
---
The Rust SDK's database remembers each account whose fingerprint a person confirmed in `members().add`, in a new table `confirmed_accounts` of its SQLite file (or in memory), so a later add of another member of that account needs no fingerprint.

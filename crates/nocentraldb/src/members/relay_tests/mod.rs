//! `members()` against the relay server on SQLite, run locally (RFC 0009,
//! "Invitations and members"; RFC 0008, "Membership entry" and "Key chain
//! entry"): what an admin writes reaches the relay, which accepts it, and
//! the other members, which fold it. They use the sync tests' support.

mod readd;

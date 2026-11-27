//! An example backend of a notes and projects app with open signup, as
//! [the guide on spaces and invitations](../../docs/spaces-and-invitations.md)
//! describes it: it signs admissions for its users (RFC 0015, "Admission
//! tokens") and keeps a directory that maps a user to their account's
//! identity and their devices' invitations, with the invitations that wait
//! for a user who has no account yet.

pub mod admission;

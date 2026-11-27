//! The directory (RFC 0015, "A user's journey"): it maps an app user, here
//! an e-mail address, to their account's identity and their devices'
//! invitations, and keeps the invitations to a user who has no account
//! yet. Nothing in it is secret: identities and invitations are public,
//! and the relay sees them anyway. What it adds to what the relay knows is
//! who the app's users are and who invited whom to which space, which is
//! the app's own data.
//!
//! It lives in memory, so it is gone when the backend stops; an app keeps
//! it in its own database.

use std::collections::{BTreeMap, BTreeSet};

/// The prefix of an invitation's text (RFC 0003, "Invitation").
const INVITATION: &str = "ncdb-invite-1:";

/// Why the directory refuses a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The user's account has another identity: one app user, one account.
    OtherIdentity,
    /// The text is no invitation, address, space id or role.
    Malformed,
    /// No such user, or one with no device yet, or no such invite.
    NotFound,
}

/// One user of the app.
#[derive(Default)]
struct User {
    /// The identity the backend admitted for the user, 64 hex digits.
    identity: Option<String>,
    /// The invitations of the user's devices, as their text.
    invitations: BTreeSet<String>,
}

/// An invitation of the app: `inviter` asked to add `email` to `space`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invite {
    /// Its id, which `done` takes.
    pub id: u64,
    /// The inviting user.
    pub inviter: String,
    /// The invited user.
    pub email: String,
    /// The space, 32 hex digits, as the app's device names it.
    pub space: String,
    /// The role: reader, writer or admin.
    pub role: String,
}

/// An invite whose invited user has devices that can be added now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ready {
    /// The invite.
    pub invite: Invite,
    /// The invitations of the invited user's devices.
    pub invitations: Vec<String>,
}

/// The directory.
#[derive(Default)]
pub struct Directory {
    users: BTreeMap<String, User>,
    invites: BTreeMap<u64, Invite>,
    next: u64,
}

impl Directory {
    /// Records `email` as a user, once.
    pub fn sign_up(&mut self, email: &str) -> Result<(), Refusal> {
        if !email.contains('@') || email.contains(char::is_whitespace) {
            return Err(Refusal::Malformed);
        }
        self.users.entry(email.to_owned()).or_default();
        Ok(())
    }

    /// Records `identity` for `email`, before the backend signs an
    /// admission for it. A recovery or a lost device keeps the identity
    /// (RFC 0015, "The account's identity"), so a second identity for the
    /// same user is refused rather than admitted beside the first.
    pub fn admit(&mut self, email: &str, identity: &str) -> Result<(), Refusal> {
        let user = self.users.get_mut(email).ok_or(Refusal::NotFound)?;
        match &user.identity {
            Some(known) if known != identity => Err(Refusal::OtherIdentity),
            _ => {
                user.identity = Some(identity.to_owned());
                Ok(())
            }
        }
    }

    /// Records the invitation of a device of `email`. The backend cannot
    /// check that it belongs to the user's account: the SDK of the admin
    /// who adds it checks its certificate, and a person may compare its
    /// fingerprint.
    pub fn add_invitation(&mut self, email: &str, invitation: &str) -> Result<(), Refusal> {
        let body = invitation
            .strip_prefix(INVITATION)
            .ok_or(Refusal::Malformed)?;
        if body.is_empty()
            || !body
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(Refusal::Malformed);
        }
        let user = self.users.get_mut(email).ok_or(Refusal::NotFound)?;
        user.invitations.insert(invitation.to_owned());
        Ok(())
    }

    /// The identity and the invitations of `email`, once a device of the
    /// user told its invitation.
    pub fn lookup(&self, email: &str) -> Result<(Option<&str>, Vec<String>), Refusal> {
        let user = self.users.get(email).ok_or(Refusal::NotFound)?;
        if user.invitations.is_empty() {
            return Err(Refusal::NotFound);
        }
        Ok((
            user.identity.as_deref(),
            user.invitations.iter().cloned().collect(),
        ))
    }

    /// Records that `inviter` invites `email` to `space` with `role`, and
    /// whether the invited user's devices can be added now. The invite
    /// stays until the inviter marks it done, also for a user who has no
    /// account yet.
    pub fn invite(
        &mut self,
        inviter: &str,
        email: &str,
        space: &str,
        role: &str,
    ) -> Result<(u64, bool), Refusal> {
        let hex = space.len() == 32
            && space
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !hex || !["reader", "writer", "admin"].contains(&role) || !email.contains('@') {
            return Err(Refusal::Malformed);
        }
        self.next += 1;
        let invite = Invite {
            id: self.next,
            inviter: inviter.to_owned(),
            email: email.to_owned(),
            space: space.to_owned(),
            role: role.to_owned(),
        };
        self.invites.insert(invite.id, invite);
        Ok((self.next, self.lookup(email).is_ok()))
    }

    /// The invites of `inviter` whose invited user has devices to add.
    pub fn ready(&self, inviter: &str) -> Vec<Ready> {
        self.invites
            .values()
            .filter(|invite| invite.inviter == inviter)
            .filter_map(|invite| {
                let (_, invitations) = self.lookup(&invite.email).ok()?;
                Some(Ready {
                    invite: invite.clone(),
                    invitations,
                })
            })
            .collect()
    }

    /// Removes the invite `id` of `inviter`, once its device added the
    /// invited user.
    pub fn done(&mut self, inviter: &str, id: u64) -> Result<(), Refusal> {
        match self.invites.get(&id) {
            Some(invite) if invite.inviter == inviter => {
                self.invites.remove(&id);
                Ok(())
            }
            _ => Err(Refusal::NotFound),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPACE: &str = "0123456789abcdef0123456789abcdef";

    fn directory() -> Directory {
        let mut directory = Directory::default();
        directory.sign_up("ana@example.com").unwrap();
        directory.sign_up("ben@example.com").unwrap();
        directory
    }

    #[test]
    fn a_user_keeps_one_identity() {
        let mut d = directory();
        d.admit("ana@example.com", &"a".repeat(64)).unwrap();
        d.admit("ana@example.com", &"a".repeat(64)).unwrap();
        assert_eq!(
            d.admit("ana@example.com", &"b".repeat(64)),
            Err(Refusal::OtherIdentity)
        );
        assert_eq!(
            d.admit("eve@example.com", &"b".repeat(64)),
            Err(Refusal::NotFound)
        );
    }

    #[test]
    fn only_an_invitation_text_is_recorded() {
        let mut d = directory();
        assert_eq!(
            d.add_invitation("ana@example.com", "hello"),
            Err(Refusal::Malformed)
        );
        assert_eq!(
            d.add_invitation("ana@example.com", "ncdb-invite-1:a b"),
            Err(Refusal::Malformed)
        );
        assert_eq!(d.lookup("ana@example.com"), Err(Refusal::NotFound));
        d.add_invitation("ana@example.com", "ncdb-invite-1:AbC-_")
            .unwrap();
        assert_eq!(
            d.lookup("ana@example.com"),
            Ok((None, vec!["ncdb-invite-1:AbC-_".to_owned()]))
        );
    }

    #[test]
    fn an_invite_to_a_user_without_a_device_waits_until_one_tells_its_invitation() {
        let mut d = directory();
        let (id, ready) = d
            .invite("ana@example.com", "ben@example.com", SPACE, "writer")
            .unwrap();
        assert!(!ready);
        assert!(d.ready("ana@example.com").is_empty());
        d.add_invitation("ben@example.com", "ncdb-invite-1:Ben")
            .unwrap();
        let ready = d.ready("ana@example.com");
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].invitations, ["ncdb-invite-1:Ben"]);
        assert!(
            d.ready("ben@example.com").is_empty(),
            "only the inviter carries it out"
        );
        assert_eq!(d.done("ben@example.com", id), Err(Refusal::NotFound));
        d.done("ana@example.com", id).unwrap();
        assert!(d.ready("ana@example.com").is_empty());
    }

    #[test]
    fn an_invite_names_a_space_id_and_a_role() {
        let mut d = directory();
        for (space, role) in [("nope", "writer"), (SPACE, "owner")] {
            let refused = d.invite("ana@example.com", "ben@example.com", space, role);
            assert_eq!(refused, Err(Refusal::Malformed));
        }
    }
}

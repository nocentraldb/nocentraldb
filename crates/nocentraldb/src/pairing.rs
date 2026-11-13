//! Accounts in the Rust SDK (RFC 0010, "Adding a device or an agent to an
//! account" and "The recovery code"; RFC 0009, "Accounts and devices"; the
//! owner's NOC-273 A, Carsten, 2026-11-12): the recovery code the app shows
//! until the person stored it, and the pairing answer a device of the
//! account hands a new device or an agent.
//!
//! The database keeps its account state and the code in its key record
//! (`crate::keys`), and keeps it again when the person confirms the code
//! and when a pairing completes.
//!
//! `pair` for a device adds it to every space where this device is an
//! admin, the account space too, and lists the account's other spaces for
//! the app to ask their admins.
//!
//! **What this does not do yet.** Pairing a database that has an account
//! of its own, which links its account, and recovery.

use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use ncdb_core::membership::{Certificate, MemberKind, Role as CoreRole};
use ncdb_core::pairing::{self as core, CheckError, LinkStatement};
use ncdb_core::recovery::{RecoveryCode as CoreCode, WORD_LIST};

use crate::db::{Db, Inner, lock};
use crate::error::{Error, ErrorCode, storage};
use crate::id::SpaceId;
use crate::keys::{self, Keys};
use crate::members::invitation::Form;
use crate::members::{AddMember, Invitation, Notice, Role};
use crate::space::Space;

/// The account of a database, from [`Db::account`]: a handle, as
/// [`Db::spaces`] is.
pub struct Account<'a> {
    pub(crate) db: &'a Db,
}

impl Account<'_> {
    /// The account's recovery code while the person has not confirmed they
    /// stored it, and `None` after (RFC 0009, "Accounts and devices"). The
    /// first open that creates an account creates its code, as does the
    /// next open of a database from before accounts; a database opened for
    /// pairing has none.
    pub async fn recovery_code(&self) -> Result<Option<RecoveryCode>, Error> {
        let inner = self.db.lock();
        Ok(inner.code.clone().map(|code| RecoveryCode {
            code,
            db: Some(self.db.inner.clone()),
        }))
    }

    /// `AccountRecovered` when this open recovered the account from its
    /// recovery code ([`OpenOptions::recover`](crate::OpenOptions::recover)),
    /// and `None` on every other open.
    pub async fn recovered(&self) -> Result<Option<Notice>, Error> {
        Ok(self.db.lock().recovered)
    }

    /// Answers the invitation of a database waiting to be paired, as a
    /// device or an agent of this account, on a device that holds the
    /// account key (RFC 0010, "Adding a device or an agent to an
    /// account"). Pair a device only after a person scanned its code or
    /// compared its fingerprint on both screens: the answer carries the
    /// account key. A database that holds no account key, an agent or one
    /// waiting to be paired itself, is refused with `NotAllowed`, as is an
    /// invitation whose member already has an account; an invitation that
    /// asks to be paired as the other kind is `InvalidValue`.
    pub async fn pair(&self, kind: PairKind, invitation: &Invitation) -> Result<Paired, Error> {
        let (answer, certified, plan) = {
            let mut inner = self.db.lock();
            let keys::Account::Holds { key, statement } = &inner.account else {
                return Err(Error::new(
                    ErrorCode::NotAllowed,
                    "this database holds no account key, so it pairs nobody",
                ));
            };
            let kind = kind.member_kind();
            // A database with an account of its own is linked into this
            // one: its device's answer carries the account key's half of
            // the link statement (RFC 0010, "Linking accounts drawn per
            // database"). An agent's is not built yet: it has no place to
            // keep the statement (pm, NOC-267).
            let (asked, old) = match &invitation.form {
                Form::Waiting(asked) => (*asked, None),
                Form::Certified(certificate) => {
                    if certificate.kind != MemberKind::Device || kind != MemberKind::Device {
                        return Err(Error::new(
                            ErrorCode::NotAllowed,
                            "the invitation's agent already has an account: linking an agent is not built yet",
                        ));
                    }
                    if certificate.member != invitation.key.id()
                        || !certificate.verify(&certificate.account_key)
                    {
                        return Err(Error::new(
                            ErrorCode::InvalidCertificate,
                            "the invitation's certificate does not hold",
                        ));
                    }
                    if crate::members::Fingerprint::of(certificate)
                        == Invitation::of(&inner).fingerprint()
                    {
                        return Err(Error::new(
                            ErrorCode::NotAllowed,
                            "the invitation's member is a device of this account already",
                        ));
                    }
                    (certificate.kind, Some(certificate.account_key))
                }
            };
            if asked != kind {
                return Err(Error::new(
                    ErrorCode::InvalidValue,
                    "the invitation asks to be paired as the other kind",
                ));
            }
            let answer = core::PairingAnswer::build(key, statement, &invitation.key, kind, old)
                .map_err(|_| {
                    Error::new(
                        ErrorCode::InvalidValue,
                        "the account key cannot be sealed to the invitation's member",
                    )
                })?;
            // The new member's certificate, as its pairing answer carries
            // it, with which this device adds it to spaces.
            let certificate =
                Certificate::sign(key, invitation.key.id(), kind).with_statement(statement.clone());
            let certified = Invitation {
                key: invitation.key,
                form: Form::Certified(certificate),
            };
            let plan = match kind {
                MemberKind::Device => {
                    // A device of an account that has no account space
                    // yet, as a guest that joined only others' spaces,
                    // creates it now: the new device joins it, and the
                    // relay admits it for an account that is a member of
                    // an admitted space (NOC-283).
                    crate::account::create(&mut inner)?;
                    crate::account::reconcile(&mut inner)?;
                    spaces_to_join(&inner)?
                }
                MemberKind::Agent | MemberKind::SubAgent => Plan::default(),
            };
            (PairingAnswer(answer), certified, plan)
        };
        let options = || AddMember {
            role: Role::Admin,
            fingerprint: None,
        };
        let mut added = Vec::new();
        for (space, app) in plan.add {
            let handle = Space::handle(self.db.inner.clone(), space);
            handle.members().add(&certified, options()).await?;
            if app {
                added.push((space, Notice::DEVICE_ADDED));
            }
        }
        Ok(Paired {
            answer,
            added,
            ask_admins: plan.ask_admins,
        })
    }

    /// Completes the pairing of this database with the answer a device of
    /// the account gave: it checks that the answer is this member's and
    /// that the account's statement and certificate hold, keeps the
    /// certificate and, for a device, the account key, and returns
    /// `DevicePaired` or `AgentPaired`. An answer that does not hold is
    /// `InvalidCertificate` and changes nothing; a database that is not
    /// waiting to be paired is `NotAllowed`.
    pub async fn complete_pairing(&self, answer: &PairingAnswer) -> Result<Vec<Notice>, Error> {
        let (keys, home, notices, link) = {
            let mut inner = self.db.lock();
            // A database with an account of its own is linked: its old key
            // signs its half of the link statement, then is deleted.
            let old = match &inner.account {
                keys::Account::None => None,
                keys::Account::Holds { key, .. } if inner.kind == MemberKind::Device => {
                    Some(key.clone())
                }
                _ => {
                    return Err(Error::new(
                        ErrorCode::NotAllowed,
                        "this database is not waiting to be paired",
                    ));
                }
            };
            if answer.0.kind() != inner.kind {
                return Err(check_error(CheckError::Certificate));
            }
            let old_key = old.as_ref().map(|old| old.verifying_key().to_bytes());
            let paired = answer
                .0
                .complete(&inner.member.key(), old_key.as_ref())
                .map_err(check_error)?;
            let statement = *paired
                .certificate
                .statement
                .clone()
                .expect("a completed answer carries the statement");
            let (account, notice) = match paired.account {
                Some(key) => (
                    keys::Account::Holds { key, statement },
                    Notice::DEVICE_PAIRED,
                ),
                None => (
                    keys::Account::Agent {
                        certificate_signature: paired.certificate.signature,
                        statement,
                    },
                    Notice::AGENT_PAIRED,
                ),
            };
            // The whole link statement is kept beside the log before the
            // old key is deleted, so that it reaches the account space.
            let link = match (&old, paired.link) {
                (Some(old), Some(half)) => {
                    let statement = paired
                        .certificate
                        .statement
                        .as_deref()
                        .expect("a statement");
                    let bytes = LinkStatement::sign(old, statement, half).encode();
                    inner.view.put_pending_link(&bytes).map_err(storage)?;
                    Some(bytes)
                }
                _ => None,
            };
            let keys = Keys {
                member: inner.member.clone(),
                account,
                code: None,
                upgraded: false,
            };
            let mut notices = vec![notice];
            if link.is_some() {
                notices.push(Notice::ACCOUNT_LINKED);
            }
            (keys, inner.home.clone(), notices, link)
        };
        if let Err(error) = home.save(&keys).await {
            if let Some(bytes) = link {
                let _ = lock(&self.db.inner).view.delete_pending_link(&bytes);
            }
            return Err(error);
        }
        let mut inner = lock(&self.db.inner);
        inner.account = keys.account;
        inner.code = None;
        if link.is_some() {
            // It joined the account: a device of it added it to the account
            // space, so it creates none of its own.
            inner.joins_account = true;
            let _ = crate::account::reconcile(&mut inner);
        }
        Ok(notices)
    }
}

/// The spaces a new device of the account joins when this device pairs
/// it, and those whose admins the app asks (RFC 0010, "Which spaces a new
/// device joins").
#[derive(Default)]
struct Plan {
    /// Each space where this device is an admin, and whether it is an app
    /// space: the account space joins too, without a notice.
    add: Vec<(SpaceId, bool)>,
    /// The app spaces of the account where this device is no admin: the
    /// ones it holds, and those the account space records for the
    /// account's other devices that it does not hold.
    ask_admins: Vec<SpaceId>,
}

fn spaces_to_join(inner: &Inner) -> Result<Plan, Error> {
    let me = inner.member.key().id();
    let account_space = crate::account::space(inner);
    let mut plan = Plan::default();
    for (space, state) in &inner.spaces {
        let account = Some(*space) == account_space;
        if !account && state.checks.model_id() != Some(inner.model_id) {
            continue;
        }
        let role = state.checks.resolved().role(me);
        if role == Some(CoreRole::Admin) {
            plan.add.push((SpaceId(*space), !account));
        } else if role.is_some() && !account {
            plan.ask_admins.push(SpaceId(*space));
        }
    }
    if account_space.is_some() {
        for record in crate::account::memberships(inner)? {
            let space = ncdb_core::ids::SpaceId(record.space);
            if record.member != me.0 && !inner.spaces.contains_key(&space) {
                plan.ask_admins.push(SpaceId(space));
            }
        }
    }
    plan.ask_admins.sort();
    plan.ask_admins.dedup();
    Ok(plan)
}

impl PairKind {
    fn member_kind(self) -> MemberKind {
        match self {
            PairKind::Device => MemberKind::Device,
            PairKind::Agent => MemberKind::Agent,
        }
    }
}

/// What a device of the account pairs: a person's device, which receives
/// the account key, or an agent, which receives a certificate only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PairKind {
    /// A person's device.
    Device,
    /// An agent.
    Agent,
}

/// What pairing returns on the device of the account: the answer to show
/// the new device, one notice per space the SDK added the new device to,
/// and the spaces where no device of the account is an admin, whose admins
/// the app asks. For an agent, `added` and `ask_admins` are empty.
#[derive(Clone, Debug)]
pub struct Paired {
    /// The answer, to show as a QR code or text.
    pub answer: PairingAnswer,
    /// The spaces the new device was added to, each with its notice.
    pub added: Vec<(SpaceId, Notice)>,
    /// The spaces whose admins the app asks to add the new device.
    pub ask_admins: Vec<SpaceId>,
}

/// A pairing answer (RFC 0010, "The pairing answer"): public data with a
/// text form, as an invitation is. `Display` writes `ncdb-pair-1:` and
/// unpadded base64url of its layout, and `FromStr` reads it, failing with
/// `InvalidValue`. It carries the account key sealed to one device, which
/// only that device opens, so it is not redacted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairingAnswer(pub(crate) core::PairingAnswer);

impl fmt::Display for PairingAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Reads an answer's text. A text off the layout is refused with
/// `InvalidValue`, and nothing is read from it; the message names what is
/// wrong, never a byte.
impl FromStr for PairingAnswer {
    type Err = Error;

    fn from_str(text: &str) -> Result<PairingAnswer, Error> {
        text.parse()
            .map(PairingAnswer)
            .map_err(|why: core::ReadError| {
                Error::new(
                    ErrorCode::InvalidValue,
                    format!("the text is not a pairing answer: {why}"),
                )
            })
    }
}

/// The error of an answer that does not hold for this member:
/// `InvalidCertificate`, which may mean a forged or swapped answer.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn check_error(why: CheckError) -> Error {
    Error::new(
        ErrorCode::InvalidCertificate,
        format!("the pairing answer does not hold for this database: {why}"),
    )
}

/// The account's recovery code, held until the person stored it (RFC
/// 0010, "The recovery code"). `Display` and `Debug` print
/// `RecoveryCode(redacted)`, and it implements no serde trait, so no
/// serializer can put it into a log. [`RecoveryCode::words`] is the one way
/// to read it.
pub struct RecoveryCode {
    code: CoreCode,
    /// The database that keeps the code, which `confirm` tells to forget
    /// it.
    db: Option<Arc<Mutex<Inner>>>,
}

impl RecoveryCode {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn new(code: CoreCode) -> RecoveryCode {
        RecoveryCode { code, db: None }
    }

    /// The twelve words, from the BIP-39 English list, for the person to
    /// write down or store in a password manager.
    pub fn words(&self) -> [&'static str; 12] {
        let words = self.code.words();
        let mut out = [""; 12];
        for (slot, word) in out.iter_mut().zip(words.split(' ')) {
            *slot = WORD_LIST
                .lines()
                .find(|listed| *listed == word)
                .expect("a recovery code's words are on the list");
        }
        out
    }

    /// Confirms that the person stored the code: the database forgets it
    /// and keeps only the recovery key's public half, in its statement
    /// (RFC 0010, "The recovery code"). Every later open returns no code.
    pub async fn confirm(self) -> Result<(), Error> {
        let Some(db) = self.db else {
            return Ok(());
        };
        let (keys, home) = {
            let inner = lock(&db);
            let keys = Keys {
                member: inner.member.clone(),
                account: inner.account.clone(),
                code: None,
                upgraded: false,
            };
            (keys, inner.home.clone())
        };
        home.save(&keys).await?;
        lock(&db).code = None;
        Ok(())
    }
}

impl fmt::Display for RecoveryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryCode(redacted)")
    }
}

impl fmt::Debug for RecoveryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryCode(redacted)")
    }
}

#[cfg(test)]
#[path = "pairing_tests.rs"]
mod tests;

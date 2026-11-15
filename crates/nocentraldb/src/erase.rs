//! Erasing a person in every space of the database (NOC-325; Carsten,
//! 2026-11-15, on NOC-326; RFC 0003, "Shredding a person's key"; RFC 0008,
//! "Shred entry"). Three calls, and two texts the app carries between the
//! devices, as it carries invitations and pairing answers (Q3 a):
//!
//! 1. [`Db::erasure_request`] names the person by one of the person's
//!    objects (Q2 a) and lists, for each space of the database where the
//!    member may write and the person has a key (Q5 a), the space, the
//!    person-key id and the performer: an [`ErasureRequest`], whose text is
//!    `ncdb-erase-1:` and unpadded base64url of its layout. It holds no
//!    person id and nothing personal.
//! 2. [`Account::sign_off`] on a device of another account, or of the
//!    performer's own account in a space whose members all belong to it
//!    (Q4), signs each space where its account may sign off by RFC 0008,
//!    checked against its own copy of the space, and a space it does not
//!    hold only for an agent of its own account, by the certificate the
//!    request carries (pm, NOC-325); it names the others:
//!    [`SignOffs`], whose text is `ncdb-sign-off-1:` and unpadded
//!    base64url of its layout.
//! 3. [`Db::erase`] checks every sign-off against the space first, and
//!    refuses the call with nothing written when one would not hold; then
//!    writes one shred per signed person key and returns [`Erased`]: the
//!    shreds, the spaces it skipped and why, and the notice `Erased`
//!    (Q6).
//!
//! The layouts: a request is version 1 (one byte), the performer's member
//! id (32 bytes), the length of its certificate (two bytes, big-endian) and
//! the certificate, as RFC 0003 encodes it, the number of entries (two
//! bytes), and per
//! entry the space id and the person-key id (16 bytes each), in order of
//! space and person key. Sign-offs are version 1, the number of entries,
//! and per entry the space id, the person-key id, the account key (32
//! bytes) and the signature (64 bytes), in the same order.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use data_encoding::BASE64URL_NOPAD;
use ncdb_core::ids::{MemberId as CoreMemberId, SpaceId as CoreSpaceId};
use ncdb_core::membership::{Certificate, MemberKind, Resolved, Role, SignOff, SignedOff};

use crate::db::{Db, lock};
use crate::error::{Error, ErrorCode};
use crate::id::{ChangeId, SpaceId};
use crate::keys;
use crate::members::{Notice, NoticeCode};
use crate::object::Object;
use crate::pairing::Account;

const REQUEST: &str = "ncdb-erase-1:";
const SIGN_OFFS: &str = "ncdb-sign-off-1:";
const VERSION: u8 = 1;

/// The notice of an erasure (Carsten, 2026-11-15, on NOC-326, Q6).
pub(crate) const ERASED: Notice = Notice {
    code: NoticeCode::Erased,
    text: "The person's personal fields are erased in these spaces. \
           Devices that never come back online, copies made outside the database \
           and unmarked free text keep them; other devices and agents of this \
           account erase only when they run the erasure too.",
};

/// What an erasure asks to sign off: per space, the person keys of one
/// person and the member that will shred them. It holds no person id and
/// nothing personal, so it may travel any way the app likes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ErasureRequest {
    performer: CoreMemberId,
    /// The performer's certificate, public as in an invitation, so that a
    /// device of its account can sign off a space of agents it does not
    /// hold (pm, NOC-325).
    certificate: Option<Certificate>,
    /// Each space with a person key of the person, in order.
    keys: BTreeSet<(CoreSpaceId, [u8; 16])>,
}

impl ErasureRequest {
    /// The spaces the request names.
    pub fn spaces(&self) -> BTreeSet<SpaceId> {
        self.keys.iter().map(|(space, _)| SpaceId(*space)).collect()
    }

    fn bytes(&self) -> Vec<u8> {
        let mut bytes = vec![VERSION];
        bytes.extend(self.performer.0);
        let certificate = self
            .certificate
            .as_ref()
            .map(Certificate::encode)
            .unwrap_or_default();
        bytes.extend((certificate.len() as u16).to_be_bytes());
        bytes.extend(certificate);
        bytes.extend((self.keys.len() as u16).to_be_bytes());
        for (space, key) in &self.keys {
            bytes.extend(space.0);
            bytes.extend(key);
        }
        bytes
    }

    fn read(bytes: &[u8]) -> Result<ErasureRequest, &'static str> {
        let (&version, rest) = bytes.split_first().ok_or("it is empty")?;
        if version != VERSION {
            return Err("its version is not 1");
        }
        let (performer, rest) = rest.split_at_checked(32).ok_or("it is too short")?;
        let (length, rest) = rest.split_at_checked(2).ok_or("it is too short")?;
        let length = u16::from_be_bytes([length[0], length[1]]) as usize;
        let (certificate, rest) = rest.split_at_checked(length).ok_or("it is too short")?;
        let certificate = match certificate.is_empty() {
            true => None,
            false => {
                Some(Certificate::decode(certificate).map_err(|_| "its certificate is not one")?)
            }
        };
        let (count, mut rest) = rest.split_at_checked(2).ok_or("it is too short")?;
        let count = u16::from_be_bytes([count[0], count[1]]) as usize;
        if rest.len() != count * 32 {
            return Err("its length does not match its number of entries");
        }
        let mut keys = BTreeSet::new();
        while let Some((entry, next)) = rest.split_at_checked(32) {
            let space = CoreSpaceId(entry[..16].try_into().expect("16 bytes"));
            keys.insert((space, entry[16..].try_into().expect("16 bytes")));
            rest = next;
        }
        if keys.len() != count {
            return Err("it names an entry twice");
        }
        Ok(ErasureRequest {
            performer: CoreMemberId(performer.try_into().expect("32 bytes")),
            certificate,
            keys,
        })
    }
}

/// `ncdb-erase-1:` and unpadded base64url of the layout.
impl fmt::Display for ErasureRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{REQUEST}{}", BASE64URL_NOPAD.encode(&self.bytes()))
    }
}

/// Reads a request's text; a text off the layout is refused with
/// `InvalidValue`.
impl FromStr for ErasureRequest {
    type Err = Error;

    fn from_str(text: &str) -> Result<ErasureRequest, Error> {
        let invalid = |why: &str| {
            Error::new(
                ErrorCode::InvalidValue,
                format!("the text is not an erasure request: {why}"),
            )
        };
        let body = text
            .strip_prefix(REQUEST)
            .ok_or_else(|| invalid("it does not start with ncdb-erase-1:"))?;
        let bytes = BASE64URL_NOPAD
            .decode(body.as_bytes())
            .map_err(|_| invalid("it is not unpadded base64url"))?;
        ErasureRequest::read(&bytes).map_err(invalid)
    }
}

/// The sign-offs of an erasure request, one per space and person key that
/// the signing account may sign off (RFC 0003, "Account sign-off of a
/// shred"). They name no person.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct SignOffs(BTreeMap<(CoreSpaceId, [u8; 16]), SignOff>);

impl SignOffs {
    fn bytes(&self) -> Vec<u8> {
        let mut bytes = vec![VERSION];
        bytes.extend((self.0.len() as u16).to_be_bytes());
        for ((space, key), sign_off) in &self.0 {
            bytes.extend(space.0);
            bytes.extend(key);
            bytes.extend(sign_off.account_key);
            bytes.extend(sign_off.signature);
        }
        bytes
    }

    fn read(bytes: &[u8]) -> Result<SignOffs, &'static str> {
        let (&version, rest) = bytes.split_first().ok_or("it is empty")?;
        if version != VERSION {
            return Err("its version is not 1");
        }
        let (count, mut rest) = rest.split_at_checked(2).ok_or("it is too short")?;
        let count = u16::from_be_bytes([count[0], count[1]]) as usize;
        if rest.len() != count * 128 {
            return Err("its length does not match its number of entries");
        }
        let mut sign_offs = BTreeMap::new();
        while let Some((entry, next)) = rest.split_at_checked(128) {
            let space = CoreSpaceId(entry[..16].try_into().expect("16 bytes"));
            let key: [u8; 16] = entry[16..32].try_into().expect("16 bytes");
            let sign_off = SignOff {
                account_key: entry[32..64].try_into().expect("32 bytes"),
                signature: entry[64..].try_into().expect("64 bytes"),
            };
            sign_offs.insert((space, key), sign_off);
            rest = next;
        }
        if sign_offs.len() != count {
            return Err("it names an entry twice");
        }
        Ok(SignOffs(sign_offs))
    }
}

impl fmt::Debug for SignOffs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignOffs")
            .field("count", &self.0.len())
            .finish()
    }
}

/// `ncdb-sign-off-1:` and unpadded base64url of the layout.
impl fmt::Display for SignOffs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{SIGN_OFFS}{}", BASE64URL_NOPAD.encode(&self.bytes()))
    }
}

/// Reads sign-offs' text; a text off the layout is refused with
/// `InvalidValue`.
impl FromStr for SignOffs {
    type Err = Error;

    fn from_str(text: &str) -> Result<SignOffs, Error> {
        let invalid = |why: &str| {
            Error::new(
                ErrorCode::InvalidValue,
                format!("the text is not a set of sign-offs: {why}"),
            )
        };
        let body = text
            .strip_prefix(SIGN_OFFS)
            .ok_or_else(|| invalid("it does not start with ncdb-sign-off-1:"))?;
        let bytes = BASE64URL_NOPAD
            .decode(body.as_bytes())
            .map_err(|_| invalid("it is not unpadded base64url"))?;
        SignOffs::read(&bytes).map_err(invalid)
    }
}

/// What [`Account::sign_off`] signed, and the spaces it did not.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Signed {
    /// The sign-offs, for the performer's [`Db::erase`].
    pub sign_offs: SignOffs,
    /// The spaces of the request this account may not sign off, or that
    /// this database does not hold, in the order of their ids.
    pub not_signed: Vec<SpaceId>,
}

/// One space a [`Db::erase`] erased the person in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ErasedIn {
    /// The space.
    pub space: SpaceId,
    /// The shred's change id, as `space.erasures().list()` names it.
    pub shred: ChangeId,
    /// Whether the erasure is provisional: the performer's own account
    /// signed it off, in a space whose members all belong to it.
    pub provisional: bool,
}

/// Why [`Db::erase`] skipped a space of the request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Skipped {
    /// No sign-off came for a person key of the space.
    NoSignOff,
    /// The member may not write in the space, or no longer holds it.
    NotWriter,
    /// The person has no key there that is not erased already.
    NoKey,
}

/// What [`Db::erase`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Erased {
    /// One per shred written, in the order of the spaces' ids.
    pub shreds: Vec<ErasedIn>,
    /// The spaces of the request it did not erase in, and why.
    pub skipped: Vec<(SpaceId, Skipped)>,
    /// The notice `Erased`.
    pub notice: Notice,
}

/// A space `Db::erase` writes shreds in: its person keys with their
/// sign-offs, and whether the shreds are provisional.
type Planned = (CoreSpaceId, Vec<([u8; 16], SignOff)>, bool);

/// Whether the request's performer is an agent of the account `account`,
/// by the certificate the request carries: it names the performer, is an
/// agent's, and is signed with `account` (pm, NOC-325, on the coding
/// agent's proposal, carrying out the owner's NOC-326 Q4). A device signs
/// a space it does not hold only for such a performer: its sign-off holds
/// only where every member belongs to the account, and makes the erasure
/// provisional; the performer's `Db::erase` checks it before it writes.
/// A sub-agent's certificate is signed by its parent, so it does not
/// qualify.
fn own_agent(request: &ErasureRequest, account: &ed25519_dalek::SigningKey) -> bool {
    let account = account.verifying_key().to_bytes();
    request.certificate.as_ref().is_some_and(|certificate| {
        certificate.member == request.performer
            && certificate.kind == MemberKind::Agent
            && certificate.account_key == account
            && certificate.verify(&account)
    })
}

/// Whether the person key `key` of `resolved` exists and is not erased.
fn live(resolved: &Resolved, key: [u8; 16]) -> bool {
    resolved.person_key(key).is_some_and(|held| !held.erased())
}

impl Db {
    /// The request to erase the person of `person`, one of the person's
    /// objects (Carsten, 2026-11-15, on NOC-326, Q2 a): every space of this
    /// database where its member may write and the person has a key that
    /// is not erased (Q5 a). An object that is no person's, one with no
    /// personal field, is refused with `NoPerson`.
    pub async fn erasure_request(&self, person: &Object) -> Result<ErasureRequest, Error> {
        let person = person.person_id().ok_or_else(|| {
            Error::new(
                ErrorCode::NoPerson,
                "the object is no person's: it has no personal field",
            )
        })?;
        let inner = lock(&self.inner);
        let me = inner.member.key().id();
        let mut keys = BTreeSet::new();
        for (space, state) in &inner.spaces {
            let resolved = state.checks.resolved();
            if resolved.role(me).is_none_or(|role| role < Role::Writer) {
                continue;
            }
            let found = inner
                .view
                .person_key_ids(*space, person)
                .map_err(crate::error::storage)?;
            for key in found.into_iter().filter(|key| live(resolved, *key)) {
                keys.insert((*space, key));
            }
        }
        Ok(ErasureRequest {
            performer: me,
            certificate: inner.account.certificate(me, inner.kind),
            keys,
        })
    }

    /// Erases the person of `request` (Carsten, 2026-11-15, on NOC-326):
    /// one shred per person key that `sign_offs` sign off, in every space
    /// of the request. Every sign-off is checked against this copy of its
    /// space first; one that would not hold refuses the call with
    /// `InvalidValue`, and nothing is written. A request of another member
    /// is refused with `InvalidValue` too. A space is skipped, and named in
    /// [`Erased::skipped`], when a person key there has no sign-off, the
    /// member may no longer write there, or the person has no key there
    /// that is not erased already.
    pub async fn erase(
        &self,
        request: &ErasureRequest,
        sign_offs: &SignOffs,
    ) -> Result<Erased, Error> {
        let mut inner = lock(&self.inner);
        let me = inner.member.key().id();
        if request.performer != me {
            return Err(Error::new(
                ErrorCode::InvalidValue,
                "the erasure request is another member's",
            ));
        }
        let mut by_space: BTreeMap<CoreSpaceId, Vec<[u8; 16]>> = BTreeMap::new();
        for (space, key) in &request.keys {
            by_space.entry(*space).or_default().push(*key);
        }
        let mut plan: Vec<Planned> = Vec::new();
        let mut skipped = Vec::new();
        for (space, keys) in by_space {
            let Some(state) = inner.spaces.get(&space) else {
                skipped.push((SpaceId(space), Skipped::NotWriter));
                continue;
            };
            let resolved = state.checks.resolved();
            if resolved.role(me).is_none_or(|role| role < Role::Writer) {
                skipped.push((SpaceId(space), Skipped::NotWriter));
                continue;
            }
            let keys: Vec<[u8; 16]> = keys
                .into_iter()
                .filter(|key| live(resolved, *key))
                .collect();
            if keys.is_empty() {
                skipped.push((SpaceId(space), Skipped::NoKey));
                continue;
            }
            let mut signed = Vec::new();
            let mut provisional = false;
            for key in &keys {
                let Some(sign_off) = sign_offs.0.get(&(space, *key)) else {
                    break;
                };
                match resolved.sign_off(space, me, *key, sign_off) {
                    Some(made) => provisional |= made == SignedOff::Provisional,
                    None => {
                        return Err(Error::new(
                            ErrorCode::InvalidValue,
                            "a sign-off does not hold for this erasure in its space; \
                             nothing was erased",
                        ));
                    }
                }
                signed.push((*key, sign_off.clone()));
            }
            if signed.len() != keys.len() {
                skipped.push((SpaceId(space), Skipped::NoSignOff));
                continue;
            }
            plan.push((space, signed, provisional));
        }
        let mut shreds = Vec::new();
        for (space, signed, provisional) in plan {
            for id in crate::members::shred(&mut inner, space, signed)? {
                shreds.push(ErasedIn {
                    space: SpaceId(space),
                    shred: ChangeId(id),
                    provisional,
                });
            }
        }
        Ok(Erased {
            shreds,
            skipped,
            notice: ERASED,
        })
    }
}

impl Account<'_> {
    /// Signs off the erasure `request` with this database's account key,
    /// in each space where the account may (Carsten, 2026-11-15, on
    /// NOC-326, Q3 a and Q4): it holds an admin there and is not the
    /// performer's account, or a writer when the performer's account is
    /// the only one with an admin, or it is the performer's own account in
    /// a space whose members all belong to it, which makes the erasure
    /// provisional (RFC 0008, "Shred entry"). Each space is checked against
    /// this database's copy of it, so a space it does not hold is not
    /// signed; [`Signed::not_signed`] names those it did not sign. A
    /// database that holds no account key, an agent's or one waiting to
    /// be paired, is refused with `NotAllowed`.
    pub async fn sign_off(&self, request: &ErasureRequest) -> Result<Signed, Error> {
        let inner = lock(&self.db.inner);
        let keys::Account::Holds { key, .. } = &inner.account else {
            return Err(Error::new(
                ErrorCode::NotAllowed,
                "this database holds no account key; a device of the account signs off",
            ));
        };
        let mut sign_offs = BTreeMap::new();
        let mut not_signed = BTreeSet::new();
        for (space, person_key) in &request.keys {
            let sign_off = SignOff::sign(key, *space, *person_key, request.performer);
            let holds = match inner.spaces.get(space) {
                Some(state) => state
                    .checks
                    .resolved()
                    .sign_off(*space, request.performer, *person_key, &sign_off)
                    .is_some(),
                None => own_agent(request, key),
            };
            if holds {
                sign_offs.insert((*space, *person_key), sign_off);
            } else {
                not_signed.insert(SpaceId(*space));
            }
        }
        // A space is signed off only with every person key it has.
        sign_offs.retain(|(space, _), _| !not_signed.contains(&SpaceId(*space)));
        Ok(Signed {
            sign_offs: SignOffs(sign_offs),
            not_signed: not_signed.into_iter().collect(),
        })
    }
}

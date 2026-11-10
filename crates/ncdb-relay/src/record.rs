//! A member's row of the membership table (RFC 0007, "The backend
//! interface"): what [`ncdb_core::membership::resolve`] says about one
//! member of a space, encoded by the core, so that a backend stores opaque
//! bytes.

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::cbor::{self, Value};
use ncdb_core::change::Header;
use ncdb_core::ids::{ChangeId, MemberId};
use ncdb_core::keys::MemberPublicKey;
use ncdb_core::membership::{Certificate, Grant, MemberState, Resolved, Revocation, Role};

use crate::store::MemberRecord;

/// A member's row, decoded: its state, for each of its accepted grants the
/// accepted revocations the grant saw, and for each from generation of its
/// grants the generations of the space's generation graph that descend from
/// it. Every control change rewrites every row, so the generations are
/// those of the graph at the row's last write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberRow {
    /// Its keys, certificate, role, grants and revocations.
    pub state: MemberState,
    /// For each of its accepted grants, the accepted revocations it saw.
    pub seen: BTreeMap<ChangeId, BTreeSet<ChangeId>>,
    /// For each from generation of its grants, the generations that are it
    /// or descend from it.
    pub descendants: BTreeMap<u64, BTreeSet<u64>>,
}

/// Why a stored row does not decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordError;

/// The rows of every member, current or former, of a resolved space.
pub fn records(resolved: &Resolved) -> Vec<MemberRecord> {
    let mut members: BTreeSet<MemberId> = resolved.seniority().iter().copied().collect();
    members.extend(resolved.members().map(|(member, _)| member));
    members
        .into_iter()
        .filter_map(|member| {
            let state = resolved.member(member)?;
            let seen = state
                .grants
                .iter()
                .filter_map(|g| Some((g.change, resolved.seen().get(&g.change)?.clone())))
                .collect();
            let descendants = state
                .grants
                .iter()
                .filter_map(|g| g.from_generation)
                .map(|from| {
                    let generations = resolved.generations().map(|(id, _)| id);
                    let below = generations.filter(|id| resolved.descends(*id, from));
                    (from, below.collect())
                })
                .collect();
            Some(MemberRecord {
                member,
                current: state.role.is_some(),
                state: encode(state, &seen, &descendants),
            })
        })
        .collect()
}

/// Decodes a stored row.
pub fn decode(record: &MemberRecord) -> Result<MemberRow, RecordError> {
    let value = cbor::decode(&record.state).map_err(|_| RecordError)?;
    let [
        signing,
        agreement,
        certificate,
        role,
        grants,
        revocations,
        seen,
        descendants,
    ] = items(value)?;
    let certificate = Certificate::decode(&bytes(certificate)?).map_err(|_| RecordError)?;
    let grants = list(grants)?
        .into_iter()
        .map(|grant| {
            let [change, role, from_generation, effective] = items(grant)?;
            Ok(Grant {
                change: change_id(change)?,
                role: self::role(role)?.ok_or(RecordError)?,
                from_generation: optional(from_generation)?,
                effective: self::role(effective)?,
            })
        })
        .collect::<Result<_, RecordError>>()?;
    let revocations = list(revocations)?
        .into_iter()
        .map(|revocation| {
            let [change, role, cutoff] = items(revocation)?;
            Ok(Revocation {
                change: change_id(change)?,
                role: self::role(role)?,
                cutoff: unsigned(cutoff)?,
            })
        })
        .collect::<Result<_, RecordError>>()?;
    let seen = list(seen)?
        .into_iter()
        .map(|entry| {
            let [grant, revocations] = items(entry)?;
            let revocations = list(revocations)?.into_iter().map(change_id);
            Ok((change_id(grant)?, revocations.collect::<Result<_, _>>()?))
        })
        .collect::<Result<_, RecordError>>()?;
    let descendants = list(descendants)?
        .into_iter()
        .map(|entry| {
            let [from, generations] = items(entry)?;
            let generations = list(generations)?.into_iter().map(unsigned);
            Ok((unsigned(from)?, generations.collect::<Result<_, _>>()?))
        })
        .collect::<Result<_, RecordError>>()?;
    let state = MemberState {
        key: MemberPublicKey {
            signing: array(signing)?,
            agreement: array(agreement)?,
        },
        certificate,
        role: self::role(role)?,
        grants,
        revocations,
    };
    if state.key.id() != record.member {
        return Err(RecordError);
    }
    Ok(MemberRow {
        state,
        seen,
        descendants,
    })
}

/// The canonical bytes of a member's state and its grants' seen
/// revocations: one array of the keys, the certificate, the role, the
/// grants, the revocations, the seen sets and the descendants of each from
/// generation. A role is 0 for none.
fn encode(
    state: &MemberState,
    seen: &BTreeMap<ChangeId, BTreeSet<ChangeId>>,
    descendants: &BTreeMap<u64, BTreeSet<u64>>,
) -> Vec<u8> {
    let id = |id: &ChangeId| Value::Bytes(id.0.to_vec());
    let role = |role: Option<Role>| Value::Unsigned(role.map_or(0, Role::number));
    let grants = state.grants.iter().map(|g| {
        Value::Array(vec![
            id(&g.change),
            role(Some(g.role)),
            g.from_generation.map_or(Value::Null, Value::Unsigned),
            role(g.effective),
        ])
    });
    let revocations = state
        .revocations
        .iter()
        .map(|r| Value::Array(vec![id(&r.change), role(r.role), Value::Unsigned(r.cutoff)]));
    let seen = seen.iter().map(|(grant, revocations)| {
        Value::Array(vec![
            id(grant),
            Value::Array(revocations.iter().map(id).collect()),
        ])
    });
    cbor::encode(&Value::Array(vec![
        Value::Bytes(state.key.signing.to_vec()),
        Value::Bytes(state.key.agreement.to_vec()),
        Value::Bytes(state.certificate.encode()),
        role(state.role),
        Value::Array(grants.collect()),
        Value::Array(revocations.collect()),
        Value::Array(seen.collect()),
        Value::Array(
            descendants
                .iter()
                .map(|(from, generations)| {
                    let generations = generations.iter().map(|g| Value::Unsigned(*g));
                    Value::Array(vec![
                        Value::Unsigned(*from),
                        Value::Array(generations.collect()),
                    ])
                })
                .collect(),
        ),
    ]))
}

fn list(value: Value) -> Result<Vec<Value>, RecordError> {
    match value {
        Value::Array(items) => Ok(items),
        _ => Err(RecordError),
    }
}

fn items<const N: usize>(value: Value) -> Result<[Value; N], RecordError> {
    list(value)?.try_into().map_err(|_| RecordError)
}

fn bytes(value: Value) -> Result<Vec<u8>, RecordError> {
    match value {
        Value::Bytes(bytes) => Ok(bytes),
        _ => Err(RecordError),
    }
}

fn array<const N: usize>(value: Value) -> Result<[u8; N], RecordError> {
    bytes(value)?.try_into().map_err(|_| RecordError)
}

fn change_id(value: Value) -> Result<ChangeId, RecordError> {
    Ok(ChangeId(array(value)?))
}

fn unsigned(value: Value) -> Result<u64, RecordError> {
    match value {
        Value::Unsigned(n) => Ok(n),
        _ => Err(RecordError),
    }
}

fn optional(value: Value) -> Result<Option<u64>, RecordError> {
    match value {
        Value::Null => Ok(None),
        other => unsigned(other).map(Some),
    }
}

fn role(value: Value) -> Result<Option<Role>, RecordError> {
    match unsigned(value)? {
        0 => Ok(None),
        n => Role::from_number(n).map(Some).ok_or(RecordError),
    }
}

impl MemberRow {
    /// RFC 0008's membership verdict for a sealed change of this member:
    /// the core's [`MemberState::admits`], over the generation graph the
    /// row holds.
    pub fn admits(&self, header: &Header) -> bool {
        self.state.admits(header, &self.seen, |generation, from| {
            self.descendants
                .get(&from)
                .is_some_and(|below| below.contains(&generation))
        })
    }
}

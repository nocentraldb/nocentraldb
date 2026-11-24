//! A write whose change would exceed the relay's change limit (NOC-421;
//! NOC-431, B′). The relay accepts no frame above 16 MiB (RFC 0007, "Large
//! changes"), so such a change would stay on the writing device for good.
//! Instead the write is split, at object boundaries only, into several
//! changes, each within the limit: every operation of one object stays in
//! one change. Each part depends on the one before it, and the writer folds
//! them all in one SQLite transaction, so on the writing device the write
//! stays atomic. On other devices the parts arrive and fold one by one, so
//! another member can see part of the write before the rest arrives, and a
//! part the relay refuses leaves the others standing. Convergence holds:
//! every member folds the same changes to the same state.
//!
//! One operation, or one object's operations together, that do not fit
//! into one change are refused with `TooLarge` before anything is written.

use std::collections::BTreeSet;

use ncdb_core::change::{Change, Operation, Payload, WriterModel};
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::ids::{ChangeId, ObjectId as CoreObjectId, SpaceId};
use ncdb_core::keys::{MemberKey, SpaceKey};
use ncdb_core::log::State;
use ncdb_core::seal;
use ncdb_core::view::View;

use crate::error::{Error, ErrorCode, storage};
use crate::id::ObjectId;

/// The largest frame the relay accepts, in bytes (RFC 0007, "Large
/// changes").
pub(crate) const CHANGE_LIMIT: usize = 16 * 1024 * 1024;

/// What a frame holds besides its payload's canonical bytes, at most: the
/// header with its ids and numbers, the signature, the AEAD nonce and tag,
/// the padding to a multiple of [`seal::PADDING_BLOCK`], and the CBOR heads
/// around them. The tests seal a part filled to the budget and check that
/// its frame stays within the limit.
const FRAME_ROOM: usize = 1024 + 2 * seal::PADDING_BLOCK;

/// The most bytes the head of the operations array takes, for any count.
const ARRAY_HEAD: usize = 9;

/// The operations of one change of a write, in the order they are sealed.
pub(crate) type Part = Vec<Operation>;

/// The bytes a part's operations may take: the limit, less the frame's
/// room and `base`, the payload's bytes without its operations.
fn budget(base: usize) -> usize {
    CHANGE_LIMIT.saturating_sub(FRAME_ROOM + base + ARRAY_HEAD)
}

/// The bytes of a payload without operations, for a change that depends
/// on `heads` changes, at the highest clock, of the model `writer`: an
/// upper bound of what a part's payload takes besides its operations.
pub(crate) fn base_length(heads: usize, writer: WriterModel) -> usize {
    let heads = (0..heads.max(1) as u64)
        .map(|n| {
            let mut id = [0xff; 32];
            id[..8].copy_from_slice(&n.to_be_bytes());
            ChangeId(id)
        })
        .collect();
    let mut payload = Payload::new(Hlc(u64::MAX), heads, Vec::new());
    payload.set_writer_model(writer);
    payload.unpadded_length()
}

/// The parts of a write of `operations`, whose payload takes `base` bytes
/// besides them. A write that fits into one change stays one part, its
/// operations as they are. A larger one is split between objects: the
/// operations are grouped by object, in the order each object first
/// appears, each object's in their own order, and the groups are filled
/// into parts in that order. An operation, or an object, that does not fit
/// into one part is `TooLarge`.
pub(crate) fn plan(operations: Vec<Operation>, base: usize) -> Result<Vec<Part>, Error> {
    let budget = budget(base);
    let mut scratch = Payload::new(Hlc(0), BTreeSet::new(), operations);
    let empty = Payload::new(Hlc(0), BTreeSet::new(), Vec::new()).unpadded_length();
    if scratch.unpadded_length() - empty <= budget {
        return Ok(vec![scratch.operations]);
    }
    let operations = std::mem::take(&mut scratch.operations);
    let mut groups: Vec<(CoreObjectId, Vec<Operation>, usize)> = Vec::new();
    let mut group_of = std::collections::BTreeMap::new();
    for operation in operations {
        let (operation, length) = measured(&mut scratch, empty, operation);
        if length > budget {
            return Err(operation_too_large(&operation, length));
        }
        let at = *group_of.entry(operation.object).or_insert_with(|| {
            groups.push((operation.object, Vec::new(), 0));
            groups.len() - 1
        });
        let group = &mut groups[at];
        group.2 += length;
        if group.2 > budget {
            return Err(object_too_large(&operation, group.2));
        }
        group.1.push(operation);
    }
    let mut parts = vec![Vec::new()];
    let mut filled = 0;
    for (_, operations, length) in groups {
        if filled + length > budget {
            parts.push(Vec::new());
            filled = 0;
        }
        filled += length;
        parts
            .last_mut()
            .expect("one part at least")
            .extend(operations);
    }
    Ok(parts)
}

/// The bytes `operation` takes in a payload, counted in `scratch`, an
/// empty payload of `empty` bytes: one operation's array head takes a byte,
/// as an empty array's does.
fn measured(scratch: &mut Payload, empty: usize, operation: Operation) -> (Operation, usize) {
    scratch.operations.push(operation);
    let length = scratch.unpadded_length() - empty;
    let operation = scratch.operations.pop().expect("pushed above");
    (operation, length)
}

fn operation_too_large(operation: &Operation, length: usize) -> Error {
    let field = operation.field.as_deref().unwrap_or("(create)");
    Error::new(
        ErrorCode::TooLarge,
        crate::texts::error::operation_too_large(
            field,
            ObjectId(operation.object),
            &operation.collection,
            length,
            CHANGE_LIMIT,
        ),
    )
}

fn object_too_large(operation: &Operation, length: usize) -> Error {
    Error::new(
        ErrorCode::TooLarge,
        crate::texts::error::object_too_large(
            ObjectId(operation.object),
            &operation.collection,
            length,
            CHANGE_LIMIT,
        ),
    )
}

/// What sealing the parts of a write takes besides them.
pub(crate) struct Seal<'a> {
    pub(crate) member: &'a MemberKey,
    pub(crate) key: &'a SpaceKey,
    pub(crate) space: SpaceId,
    pub(crate) generation: u64,
    /// The first part's author sequence; each next part takes one more.
    pub(crate) sequence: u64,
    /// The first part's dependencies; each next part depends on the part
    /// before it, which depends on them.
    pub(crate) heads: BTreeSet<ChangeId>,
    pub(crate) writer: WriterModel,
}

/// Seals each part as one change, its clock from `clock`, in order.
pub(crate) fn seal_parts(
    parts: Vec<Part>,
    seal: Seal<'_>,
    mut clock: impl FnMut() -> Result<Hlc, Error>,
) -> Result<Vec<Change>, Error> {
    let mut changes: Vec<Change> = Vec::with_capacity(parts.len());
    let mut heads = seal.heads;
    for (n, operations) in parts.into_iter().enumerate() {
        let mut payload = Payload::new(clock()?, heads, operations);
        payload.set_writer_model(seal.writer);
        let change = seal::seal(
            seal.member,
            seal.key,
            seal.space,
            seal.sequence + n as u64,
            seal.generation,
            &payload,
        );
        // The payload's operations, as large as the change's in memory,
        // go before the next part is built (NOC-422).
        drop(payload);
        heads = BTreeSet::from([change.id()]);
        changes.push(change);
    }
    Ok(changes)
}

/// Appends the member's own changes to the log, with the time they were
/// written, and folds them, all in one SQLite transaction: all or none.
/// Returns the last one's id. Every check passes on a change the member
/// wrote itself, so a change that does not fold is a bug of the SDK.
pub(crate) fn fold_all(
    view: &mut View,
    checks: &mut SealChecks,
    changes: Vec<Change>,
) -> Result<ChangeId, Error> {
    let space = changes
        .first()
        .expect("a write is one change at least")
        .header()
        .space;
    let ids: Vec<ChangeId> = changes.iter().map(Change::id).collect();
    view.receive_all_own_changes(changes, checks, crate::clock::now_millis())
        .map_err(storage)?;
    crate::status::note_shreds(view, space, checks.resolved()).map_err(storage)?;
    for id in &ids {
        match view.state(space, *id) {
            Some(State::Folded { parked }) if parked.is_empty() => {}
            other => panic!("the member's own change was not folded: {other:?}"),
        }
    }
    Ok(*ids.last().expect("a write is one change at least"))
}

#[cfg(test)]
#[path = "split_frame_tests.rs"]
mod frame_tests;

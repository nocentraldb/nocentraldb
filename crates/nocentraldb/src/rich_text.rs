//! What the TypeScript SDK's editor binding asks of a rich-text field
//! (RFC 0013, "An editor: the binding"; NOC-461), through the bridge: a
//! client id leased for as long as the binding writes, the field's
//! document, the part of it a binding does not hold yet, and the write of
//! an update under the lease. The Rust SDK has no editor binding of its
//! own in rule 1 (RFC 0013, "The Rust SDK"), so none of this is public.
//!
//! Built only with the compiler cfg `ncdb_rich_text` (NOC-468).

use ncdb_core::view::{Field, FieldState, Lease};
use yrs::updates::decoder::Decode;
use yrs::{ClientID, Doc, ReadTxn, StateVector, Transact, Update};

use crate::db::Inner;
use crate::error::{Error, ErrorCode, storage};
use crate::id::{ObjectId, SpaceId};
use crate::value::{Repr, Value};

/// A rich-text update to write under a lease: the value
/// [`crate::encode`] turns into operation 5.
#[derive(Clone, PartialEq)]
pub(crate) struct RichTextUpdate {
    /// The Yjs update, encoding v1.
    pub(crate) update: Vec<u8>,
    /// The leased client id, whose items the update may add.
    pub(crate) client: u64,
    /// The nonce the client id derives from.
    pub(crate) nonce: [u8; 8],
}

impl RichTextUpdate {
    /// The value that writes this update.
    pub(crate) fn value(self) -> Value {
        Value(Repr::RichText(self))
    }
}

/// One binding's hold on a field: where it is and the client id it leased.
#[derive(Clone, Debug)]
pub(crate) struct Binding {
    pub(crate) space: SpaceId,
    pub(crate) collection: String,
    pub(crate) object: ObjectId,
    pub(crate) field: String,
    pub(crate) lease: Lease,
}

/// What a binding opens with.
pub(crate) struct Opened {
    pub(crate) binding: Binding,
    /// The field's document, a Yjs update of its whole state, also when
    /// nothing is written yet.
    pub(crate) document: Vec<u8>,
    /// Whether the member may write the field: a writer or an admin that
    /// holds the space's key.
    pub(crate) writable: bool,
}

/// Leases a client id for `field` of the object `object` of `collection`
/// in `space`, which the copy must hold, and reads the field's document.
pub(crate) fn open(
    inner: &mut Inner,
    space: SpaceId,
    collection: &str,
    object: ObjectId,
    field: &str,
) -> Result<Opened, Error> {
    let document = document(inner, space, collection, object, field)?;
    let member = inner.member.key().id();
    let lease = inner
        .view
        .lease_rich_text(member, space.0, collection, object.0, field)
        .map_err(storage)?;
    let writable = crate::space::writable(inner, space).is_ok()
        && inner.spaces.get(&space.0).is_some_and(|state| {
            let role = state.checks.resolved().role(member);
            role.is_some_and(|role| role >= ncdb_core::membership::Role::Writer)
        });
    Ok(Opened {
        binding: Binding {
            space,
            collection: collection.to_owned(),
            object,
            field: field.to_owned(),
            lease,
        },
        document: load(&document)
            .transact()
            .encode_state_as_update_v1(&StateVector::default()),
        writable,
    })
}

/// The document of `field` of `object`: empty while nothing is written.
pub(crate) fn document(
    inner: &Inner,
    space: SpaceId,
    collection: &str,
    object: ObjectId,
    field: &str,
) -> Result<Vec<u8>, Error> {
    match inner.model.field(collection, field) {
        Some(Field::RichText(_)) => {}
        Some(_) => {
            return Err(Error::new(
                ErrorCode::InvalidValue,
                crate::texts::error::field_does_not_hold("rich text"),
            ));
        }
        None if inner.model.collections.contains_key(collection) => {
            return Err(Error::new(
                ErrorCode::NotFound,
                crate::texts::error::no_such_field(collection, field),
            ));
        }
        None => {
            return Err(Error::new(
                ErrorCode::NotFound,
                crate::texts::error::no_such_collection(collection),
            ));
        }
    }
    let Some(mut row) = inner
        .view
        .get(space.0, collection, object.0)
        .map_err(storage)?
    else {
        return Err(Error::new(
            ErrorCode::NotFound,
            crate::texts::error::no_such_object(object, collection),
        ));
    };
    Ok(match row.fields.remove(field) {
        Some(FieldState::RichText(state)) => state.document().to_vec(),
        _ => Vec::new(),
    })
}

/// The part of `document` that a document with `state_vector` does not
/// hold, as a Yjs update; `None` for a state vector Yjs cannot read.
pub(crate) fn missing(document: &[u8], state_vector: &[u8]) -> Option<Vec<u8>> {
    let held = StateVector::decode_v1(state_vector).ok()?;
    Some(load(document).transact().encode_state_as_update_v1(&held))
}

/// The clock `client` reached in `document`: the next clock it writes.
pub(crate) fn clock(document: &[u8], client: u64) -> u64 {
    u64::from(
        load(document)
            .transact()
            .state_vector()
            .get(&ClientID::new(client)),
    )
}

/// Gives the binding's client id back, with the clock it reached in the
/// field's document.
pub(crate) fn release(inner: &mut Inner, binding: &Binding) -> Result<(), Error> {
    let document = document(
        inner,
        binding.space,
        &binding.collection,
        binding.object,
        &binding.field,
    )
    .unwrap_or_default();
    let reached = clock(&document, binding.lease.client);
    inner
        .view
        .release_rich_text(&binding.lease, reached)
        .map_err(storage)
}

/// The core's own document bytes, which Yrs always loads.
fn load(document: &[u8]) -> Doc {
    let doc = Doc::new();
    if let Ok(update) = Update::decode_v1(document) {
        let _ = doc.transact_mut().apply_update(update);
    }
    doc
}

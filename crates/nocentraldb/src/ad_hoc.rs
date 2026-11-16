//! Ad-hoc spaces for sub-agents (NOC-318; RFC 0003, "Sub-agents get an
//! ad-hoc space"; RFC 0009, "Sub-agents and ad-hoc spaces"). A parent
//! creates a space with a fresh key and copies into it only the objects a
//! task needs, as new objects with fresh ids, so that the sub-agent never
//! learns a source's id. The parent's database keeps, on the parent only,
//! which object each copy came from (`ncdb_core::view::AdHocCopy`), and
//! finds it with [`Space::source_of`] and [`Space::copy_of`].
//!
//! What the owner decided on NOC-320 (Carsten, 2026-11-15):
//!
//! - Q4: the call returns each source and its copy, and the lookups find
//!   them later.
//! - Q6: a copied link points to the copy of its target, or is cleared.
//! - Q7: copies may come from several spaces; a deleted object is refused.
//! - Q3 and Q8: `local` and `personal` wait for tickets of their own, so
//!   both are refused for now, and personal fields are not copied.

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::change::WriterModel;
use ncdb_core::ids::{ObjectId as CoreObjectId, SpaceId as CoreSpaceId};
use ncdb_core::view::{AdHocCopy, Field};

use crate::db::{Inner, lock};
use crate::error::{Error, ErrorCode, storage};
use crate::id::{ObjectId, SpaceId};
use crate::members::Notice;
use crate::object::Object;
use crate::space::{Space, Spaces, create_space, write_change};
use crate::tx::Transaction;
use crate::value::{Repr, Value};

/// The options of [`Spaces::create_ad_hoc`] (RFC 0009, "Sub-agents and
/// ad-hoc spaces").
#[derive(Clone, Debug, Default)]
pub struct AdHoc {
    /// The objects to copy, read from any space of this database; each
    /// becomes a new object of the ad-hoc space.
    pub copy: Vec<Object>,
    /// Copies the personal fields too. Not yet: refused with `NotAllowed`
    /// until an erasure reaches a copy in another space (NOC-320 Q8).
    pub personal: bool,
    /// Keeps the space off the relay. Not yet: refused with `NotAllowed`
    /// until a ticket of its own (NOC-320 Q3).
    pub local: bool,
}

/// What [`Spaces::create_ad_hoc`] returns.
#[derive(Debug)]
pub struct AdHocCreated {
    /// The ad-hoc space.
    pub space: Space,
    /// [`NoticeCode::AdHocSpaceCreated`](crate::NoticeCode): the space
    /// limits what the sub-agent can decrypt, not what a process running
    /// as the same user can read.
    pub notice: Notice,
    /// Each source and its copy, in the order of [`AdHoc::copy`].
    pub copies: Vec<(ObjectId, ObjectId)>,
    /// How many links, one or of many, were cleared because their target
    /// was not copied.
    pub cleared_links: usize,
    /// How many set plugin fields were not copied: a plugin's read is no
    /// intent its `write` takes, so a copy leaves them unset.
    pub plugin_fields_not_copied: usize,
}

/// One source, read again under the lock, and the id of its copy.
struct Source {
    object: Object,
    copy: CoreObjectId,
}

impl Spaces<'_> {
    /// Creates an ad-hoc space for a sub-agent: a space with a fresh key,
    /// which syncs as any other, and in one change a copy of each object of
    /// `options.copy`, as a new object with a fresh id (RFC 0003,
    /// "Sub-agents get an ad-hoc space"). The parent then adds the
    /// sub-agent with [`Members::add`](crate::Members::add).
    ///
    /// A copied link points to the copy of its target when that is copied
    /// too, and is cleared otherwise, so that no source id reaches the
    /// space. Personal fields and plugin fields are not copied.
    ///
    /// A source this database does not hold is `NotFound`; a deleted one,
    /// or one named twice, `InvalidValue`; `personal` or `local`,
    /// `NotAllowed`. A refused call creates nothing.
    pub async fn create_ad_hoc(&self, options: AdHoc) -> Result<AdHocCreated, Error> {
        if options.personal {
            return Err(Error::new(
                ErrorCode::NotAllowed,
                crate::texts::error::AD_HOC_PERSONAL_LATER,
            ));
        }
        if options.local {
            return Err(Error::new(
                ErrorCode::NotAllowed,
                crate::texts::error::AD_HOC_LOCAL_LATER,
            ));
        }
        let mut inner = self.db.lock();
        let sources = sources(&inner, &options.copy)?;
        let copies: BTreeMap<(CoreSpaceId, CoreObjectId), CoreObjectId> = sources
            .iter()
            .map(|source| ((source.object.space().0, source.object.id().0), source.copy))
            .collect();
        let model = inner.model.clone();
        let mut tx = Transaction::new(&model);
        let mut cleared_links = 0;
        let mut plugin_fields_not_copied = 0;
        for source in &sources {
            let copied = values(&inner, &source.object, &copies);
            cleared_links += copied.cleared_links;
            plugin_fields_not_copied += copied.plugin_fields_not_copied;
            let values = copied
                .values
                .iter()
                .map(|(name, value)| (name.as_str(), value.clone()));
            tx.create_at(source.object.collection(), source.copy, values)?;
        }

        crate::account::create(&mut inner)?;
        let model_id = inner.model_id;
        let space = create_space(&mut inner, model_id)?;
        let writer = WriterModel {
            id: inner.model.id,
            version: inner.model.version,
        };
        write_change(&mut inner, space, tx, writer, |_| Ok(()))?;
        let recorded: Vec<AdHocCopy> = sources
            .iter()
            .map(|source| AdHocCopy {
                copy: source.copy,
                source_space: source.object.space().0,
                source: source.object.id().0,
            })
            .collect();
        inner
            .view
            .record_ad_hoc_copies(space, &recorded)
            .map_err(storage)?;
        crate::account::reconcile(&mut inner)?;
        Ok(AdHocCreated {
            space: Space::handle(self.db.inner.clone(), SpaceId(space)),
            notice: Notice::AD_HOC_SPACE_CREATED,
            copies: sources
                .iter()
                .map(|source| (source.object.id(), ObjectId(source.copy)))
                .collect(),
            cleared_links,
            plugin_fields_not_copied,
        })
    }
}

/// Each object of `copy`, read again from the view, with a fresh id for
/// its copy: refused when the database does not hold it, when it is
/// deleted, or when it is named twice.
fn sources(inner: &Inner, copy: &[Object]) -> Result<Vec<Source>, Error> {
    let mut named = BTreeSet::new();
    copy.iter()
        .map(|object| {
            let (space, id) = (object.space(), object.id());
            if !named.insert((space, id)) {
                return Err(Error::new(
                    ErrorCode::InvalidValue,
                    crate::texts::error::ad_hoc_source_twice(id),
                ));
            }
            let read = Object::read(inner, space.0, object.collection(), id.0)?;
            let Some(read) = read else {
                return Err(Error::new(
                    ErrorCode::NotFound,
                    crate::texts::error::ad_hoc_source_not_held(id, space),
                ));
            };
            if read.deleted() {
                return Err(Error::new(
                    ErrorCode::InvalidValue,
                    crate::texts::error::ad_hoc_source_deleted(id),
                ));
            }
            Ok(Source {
                object: read,
                copy: CoreObjectId::random(),
            })
        })
        .collect()
}

/// What a copy of one object is written with.
struct Copied {
    values: Vec<(String, Value)>,
    cleared_links: usize,
    plugin_fields_not_copied: usize,
}

/// The values of the copy of `object`: each set field that is neither
/// personal nor of a plugin; a link to the copy of its target, or none.
fn values(
    inner: &Inner,
    object: &Object,
    copies: &BTreeMap<(CoreSpaceId, CoreObjectId), CoreObjectId>,
) -> Copied {
    let collection = object.collection();
    let fields = &inner.model.collections[collection];
    let space = object.space().0;
    let copy_of = |value: &Value| match value.0 {
        Repr::Id(target) => copies
            .get(&(space, target.0))
            .map(|copy| Value::from(ObjectId(*copy))),
        _ => None,
    };
    let mut copied = Copied {
        values: Vec::new(),
        cleared_links: 0,
        plugin_fields_not_copied: 0,
    };
    for (name, value) in object.fields() {
        let Some(value) = value else { continue };
        if inner.model.person(collection, name).is_some() {
            continue;
        }
        let elements = match &value.0 {
            Repr::List(elements) => elements.as_slice(),
            _ => &[],
        };
        let written = match fields.get(name) {
            Some(Field::Basic(_)) => Some(value.clone()),
            Some(Field::Set(_)) if !elements.is_empty() => Some(Value::add(elements.to_vec())),
            Some(Field::Link(_)) => {
                let target = copy_of(value);
                copied.cleared_links += usize::from(target.is_none());
                target
            }
            Some(Field::Links(_)) => {
                let targets: Vec<Value> = elements.iter().filter_map(copy_of).collect();
                copied.cleared_links += elements.len() - targets.len();
                (!targets.is_empty()).then(|| Value::add(targets))
            }
            // A reference names an object of any space by its id, so it
            // is never copied.
            Some(Field::Ref(_)) => {
                copied.cleared_links += 1;
                None
            }
            Some(Field::Plugin(_)) => {
                copied.plugin_fields_not_copied += 1;
                None
            }
            _ => None,
        };
        if let Some(written) = written {
            copied.values.push((name.clone(), written));
        }
    }
    copied
}

impl Space {
    /// The space and the id of the object that `copy`, an object of this
    /// ad-hoc space, was copied from, or `None` when this database made no
    /// such copy. Only the parent that created the space knows it: the
    /// mapping never syncs.
    pub async fn source_of(&self, copy: ObjectId) -> Result<Option<(SpaceId, ObjectId)>, Error> {
        let inner = lock(self.inner());
        let source = inner
            .view
            .ad_hoc_source_of(self.id().0, copy.0)
            .map_err(storage)?;
        Ok(source.map(|(space, id)| (SpaceId(space), ObjectId(id))))
    }

    /// The copy in this ad-hoc space of the object `source` of
    /// `source_space`, or `None` when this database copied no such object
    /// into it.
    pub async fn copy_of(
        &self,
        source_space: SpaceId,
        source: ObjectId,
    ) -> Result<Option<ObjectId>, Error> {
        let inner = lock(self.inner());
        let copy = inner
            .view
            .ad_hoc_copy_of(self.id().0, source_space.0, source.0)
            .map_err(storage)?;
        Ok(copy.map(ObjectId))
    }
}

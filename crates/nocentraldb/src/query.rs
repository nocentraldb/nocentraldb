//! Queries and pages (RFC 0009, "Queries and indexes"): a query names a
//! declared index, the values of its equality prefix, a page size and an
//! optional cursor, and a page holds at most that many objects in index
//! order. There is nothing else: no filter on a field outside the index,
//! no sort of the app's choosing, no SQL (GENESIS.md, principle 6).
//!
//! A query and a cursor hold plaintext field values, so their `Debug`
//! output names only the index.

use std::borrow::Borrow;

use ncdb_core::cbor;
use ncdb_core::ids::SpaceId as CoreSpaceId;
use ncdb_core::view::{self, QueryError, View};

use crate::db::{Db, Inner, lock};
use crate::error::{Error, ErrorCode};
use crate::object::Object;
use crate::space::Space;
use crate::value::Value;

/// A query: a declared index, from [`Db::index`], the values of its
/// equality prefix, a page size and where the page starts.
#[derive(Clone)]
pub struct Query {
    collection: String,
    index: String,
    equal: Vec<Value>,
    take: usize,
    after: Option<Cursor>,
}

/// Where the page after another starts: the position after its last item.
/// It holds plaintext field values; it never leaves the device in the
/// SDK's design, and prints as `[redacted]`.
#[derive(Clone, PartialEq)]
pub struct Cursor(view::Cursor);

/// One page of a query: its objects in index order, the cursor after the
/// last one, and whether more objects follow it.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    /// The objects, in index order, each with its space.
    pub items: Vec<Object>,
    /// Where the next page starts; `None` when the page is not full.
    pub cursor: Option<Cursor>,
    /// Whether the index holds objects after this page.
    pub has_more: bool,
}

impl std::fmt::Debug for Query {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Query")
            .field("collection", &self.collection)
            .field("index", &self.index)
            .field("take", &self.take)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Cursor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cursor([redacted])")
    }
}

impl Query {
    /// Fixes the index's leading fields, as many as its equality prefix
    /// declares, no more and no fewer.
    pub fn equal<V: Into<Value>>(mut self, values: impl IntoIterator<Item = V>) -> Query {
        self.equal = values.into_iter().map(Into::into).collect();
        self
    }

    /// The page size: 50 unless set, at most 500.
    pub fn take(mut self, n: usize) -> Query {
        self.take = n;
        self
    }

    /// Starts the page after `cursor`, the cursor of a page of this query.
    pub fn after(mut self, cursor: Cursor) -> Query {
        self.after = Some(cursor);
        self
    }

    pub(crate) fn collection(&self) -> &str {
        &self.collection
    }

    /// The page of this query in the view: in `space` for an index that
    /// runs in one space, across every space otherwise.
    pub(crate) fn page(&self, inner: &Inner, space: Option<CoreSpaceId>) -> Result<Page, Error> {
        let equal: Vec<cbor::Value> = self
            .equal
            .iter()
            .map(|value| value.to_cbor().ok_or_else(|| refused(QueryError::Equality)))
            .collect::<Result<_, _>>()?;
        let after = self.after.as_ref().map(|cursor| &cursor.0);
        let page = lookup(&inner.view, space, self, &equal, after, self.take)?;
        let has_more = match &page.next {
            Some(next) => !lookup(&inner.view, space, self, &equal, Some(next), 1)?
                .rows
                .is_empty(),
            None => false,
        };
        let items = page
            .rows
            .iter()
            .map(|(space, id)| {
                Object::read(inner, *space, &self.collection, *id)?.ok_or_else(|| {
                    Error::new(ErrorCode::StorageFailed, "an index row has no object")
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Page {
            items,
            cursor: page.next.map(Cursor),
            has_more,
        })
    }
}

fn lookup(
    view: &View,
    space: Option<CoreSpaceId>,
    query: &Query,
    equal: &[cbor::Value],
    after: Option<&view::Cursor>,
    limit: usize,
) -> Result<view::Page, Error> {
    let (collection, index) = (&query.collection, &query.index);
    match space {
        None => view.page(collection, index, equal, after, limit),
        Some(space) => view.page_in(space, collection, index, equal, after, limit),
    }
    .map_err(|error| match error {
        view::Error::Query(error) => refused(error),
        error => crate::error::storage(error),
    })
}

/// The error of a query the view refused.
fn refused(error: QueryError) -> Error {
    let (code, message) = match error {
        QueryError::Cursor => (
            ErrorCode::CursorOfAnotherModel,
            "the cursor belongs to another model, index or equality prefix",
        ),
        QueryError::UnknownIndex => (ErrorCode::NotFound, "the model declares no such index"),
        QueryError::Equality => (
            ErrorCode::InvalidValue,
            "the query does not give one value per field of the index's equality prefix",
        ),
        QueryError::PageSize => (ErrorCode::InvalidValue, "the page size is 0 or above 500"),
        QueryError::Space => (
            ErrorCode::InvalidValue,
            "an index that runs in one space is read with space.find, any other with db.find",
        ),
    };
    Error::new(code, message)
}

impl Db {
    /// The declared index `name` of `collection`, as a query of its first
    /// page; `NotFound` when the model declares no such index.
    pub fn index(&self, collection: &str, name: &str) -> Result<Query, Error> {
        let inner = self.lock();
        match inner.model.index(collection, name) {
            Some(_) => Ok(Query {
                collection: collection.to_owned(),
                index: name.to_owned(),
                equal: Vec::new(),
                take: View::DEFAULT_PAGE,
                after: None,
            }),
            None => Err(Error::new(
                ErrorCode::NotFound,
                format!("the model declares no index {name} of {collection}"),
            )),
        }
    }

    /// Runs `query`, of an index that spans every space of the member.
    pub async fn find(&self, query: impl Borrow<Query>) -> Result<Page, Error> {
        query.borrow().page(&self.lock(), None)
    }
}

impl Space {
    /// Runs `query`, of an index that runs in one space, in this space.
    pub async fn find(&self, query: impl Borrow<Query>) -> Result<Page, Error> {
        query.borrow().page(&lock(self.inner()), Some(self.id().0))
    }
}

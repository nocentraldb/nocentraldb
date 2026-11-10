//! One lookup through a declared index (RFC 0006, "Declared indexes and
//! ordered pages"): the rows of the index with these values for its
//! equality prefix, after this cursor, at most N, in index order. The core
//! builds the SQL from the model; there is no raw SQL.
//!
//! The cursor is the last row's index key. The next page is the rows whose
//! key comes after it in the index's order, written out key by key so that
//! an unset value and a descending key compare as the index orders them
//! (SQLite's row values compare all keys one way, and with an unset value
//! they compare to nothing). Every index ends with the object id after the
//! space id, so the order is total and a page boundary is exact. An index
//! that runs in one space puts the space id first, as the start of its
//! equality prefix (RFC 0006, Amendment 6).

use std::fmt;

use rusqlite::types::Value as Sql;
use rusqlite::{Connection, params_from_iter};

use super::fold::sql_value;
use super::model::{Direction, Index, Scope};
use super::schema::{index_columns, index_name, prefix, rows};
use super::{Error, View};
use crate::cbor::Value;
use crate::ids::{ObjectId, SpaceId};

/// Why a lookup was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryError {
    /// The model declares no index of this name in this collection.
    UnknownIndex,
    /// The lookup does not give one value per key of the index's equality
    /// prefix.
    Equality,
    /// The page size is 0 or above [`View::MAX_PAGE`].
    PageSize,
    /// The lookup names a space and the index spans every space, or the
    /// index runs in one space and the lookup names none.
    Space,
    /// The cursor belongs to another index, another equality prefix or
    /// another model: start again from the first page.
    Cursor,
}

/// The position after the last row of a page: the row's index key, with
/// the index's collection and name and the model's hash. It holds plaintext field values,
/// so its `Debug` output shows only the index and the number of values.
#[derive(Clone, PartialEq)]
pub struct Cursor {
    collection: String,
    index: String,
    model: [u8; 32],
    /// The values of every column of the index's SQLite index, in order:
    /// its keys, the space id and the object id.
    key: Vec<Sql>,
}

impl fmt::Debug for Cursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cursor")
            .field("collection", &self.collection)
            .field("index", &self.index)
            .field("values", &self.key.len())
            .finish()
    }
}

/// One page of a lookup: the ids of its rows, in index order, and the
/// cursor after the last one when the page is full.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    /// The rows, by space id and object id.
    pub rows: Vec<(SpaceId, ObjectId)>,
    /// Where the next page starts; `None` when this page is the last.
    pub next: Option<Cursor>,
}

/// The columns after the equality prefix, each with its direction.
fn tail(index: &Index) -> Vec<(String, Direction)> {
    index_columns(index).split_off(prefix(index))
}

/// The rows whose key `tail` comes after `after`, key by key, nested so
/// that SQLite walks one index instead of a union of ranges: after a set
/// value, ascending, come the larger values; after it, descending, the
/// smaller values and then an unset one; after an unset value, ascending,
/// every set value; nothing comes after an unset value, descending, but
/// the rows that tie on it. A tie goes on to the next key.
fn after_key(tail: &[(String, Direction)], after: &[Sql]) -> (String, Vec<Sql>) {
    let Some(((name, direction), value)) = tail.first().zip(after.first()) else {
        return ("0".to_owned(), Vec::new());
    };
    let (rest, mut rest_values) = after_key(&tail[1..], &after[1..]);
    let set = *value != Sql::Null;
    let mut values = Vec::new();
    let condition = match (direction, set) {
        (Direction::Ascending, true) => {
            values.extend([value.clone(), value.clone()]);
            format!("({name} > ? OR ({name} = ? AND {rest}))")
        }
        (Direction::Ascending, false) => {
            format!("({name} IS NOT NULL OR ({name} IS NULL AND {rest}))")
        }
        (Direction::Descending, true) => {
            values.extend([value.clone(), value.clone()]);
            format!("({name} < ? OR {name} IS NULL OR ({name} = ? AND {rest}))")
        }
        (Direction::Descending, false) => format!("({name} IS NULL AND {rest})"),
    };
    values.append(&mut rest_values);
    (condition, values)
}

/// The SQL of a page, and its parameters: the equality values (the space
/// id first, when the index runs in one space), the cursor's values that
/// are set, and the page size.
fn statement(
    index: &Index,
    equal: &[Sql],
    after: Option<&[Sql]>,
    limit: usize,
) -> (String, Vec<Sql>) {
    let columns = index_columns(index);
    let tail = tail(index);
    let mut parameters: Vec<Sql> = equal.to_vec();
    // The condition of the partial index: no deleted object.
    let mut conditions: Vec<String> = std::iter::once("deleted = 0".to_owned())
        .chain(
            columns[..prefix(index)]
                .iter()
                .map(|(name, _)| format!("{name} IS ?")),
        )
        .collect();
    if let Some(after) = after {
        // A bound on the first key lets SQLite seek to the cursor; the
        // nested condition then says exactly which rows come after it.
        let (first, direction) = &tail[0];
        match (direction, &after[0]) {
            (Direction::Ascending, Sql::Null) => {}
            (Direction::Ascending, value) => {
                conditions.push(format!("{first} >= ?"));
                parameters.push(value.clone());
            }
            (Direction::Descending, Sql::Null) => conditions.push(format!("{first} IS NULL")),
            (Direction::Descending, _) => {}
        }
        let (condition, values) = after_key(&tail, after);
        conditions.push(condition);
        parameters.extend(values);
    }
    let order: Vec<String> = columns
        .iter()
        .map(|(name, direction)| match direction {
            Direction::Ascending => format!("{name} ASC"),
            Direction::Descending => format!("{name} DESC"),
        })
        .collect();
    let names: Vec<&str> = columns.iter().map(|(name, _)| name.as_str()).collect();
    let sql = format!(
        "SELECT {} FROM {} WHERE {} ORDER BY {} LIMIT ?",
        names.join(", "),
        rows(&index.collection),
        conditions.join(" AND "),
        order.join(", "),
    );
    parameters.push(Sql::Integer(i64::try_from(limit).unwrap_or(i64::MAX)));
    (sql, parameters)
}

/// The query plans of an index's first page and of a page after a
/// cursor, one line each.
pub(super) fn plans(conn: &Connection, index: &Index) -> Result<Vec<String>, Error> {
    let equal = vec![Sql::Null; prefix(index)];
    let after = vec![Sql::Integer(0); tail(index).len()];
    [None, Some(after.as_slice())]
        .into_iter()
        .map(|after| {
            let (sql, parameters) = statement(index, &equal, after, 1);
            let mut statement = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
            let details: Vec<String> = statement
                .query_map(params_from_iter(parameters), |row| row.get(3))?
                .collect::<Result<_, _>>()?;
            Ok(details.join("; "))
        })
        .collect()
}

/// Refuses a view whose lookups would not run through their declared
/// index, or would sort in a temporary B-tree (RFC 0006, "The check").
pub(super) fn check(conn: &Connection, indexes: &[Index]) -> Result<(), Error> {
    for index in indexes {
        for plan in plans(conn, index)? {
            if !fits(&plan, &index_name(index)) {
                return Err(Error::Plan(format!("{}.{}", index.collection, index.name)));
            }
        }
    }
    Ok(())
}

/// Whether a plan walks the SQLite index `index` and nothing else: every
/// step that reads the table (a SCAN or a SEARCH) goes through it, and no
/// step sorts in a temporary B-tree.
fn fits(plan: &str, index: &str) -> bool {
    let uses = |step: &str| {
        let after = |marker: &str| step.split_once(marker).map(|(_, rest)| rest);
        let name = after(" USING COVERING INDEX ").or_else(|| after(" USING INDEX "));
        name.and_then(|rest| rest.split(' ').next()) == Some(index)
    };
    let steps: Vec<&str> = plan.split("; ").collect();
    let reads: Vec<&&str> = steps
        .iter()
        .filter(|step| step.starts_with("SCAN ") || step.starts_with("SEARCH "))
        .collect();
    !reads.is_empty()
        && reads.iter().all(|step| uses(step))
        && !steps.iter().any(|step| step.contains("TEMP B-TREE"))
}

impl View {
    /// The page size a caller that names none should use.
    pub const DEFAULT_PAGE: usize = 50;
    /// The largest page, so that one read holds the database briefly.
    pub const MAX_PAGE: usize = 500;

    /// One page of the declared index `index` of `collection`, which spans
    /// every space: the rows whose equality prefix holds `equal`, after
    /// `after`, at most `limit`.
    pub fn page(
        &self,
        collection: &str,
        index: &str,
        equal: &[Value],
        after: Option<&Cursor>,
        limit: usize,
    ) -> Result<Page, Error> {
        self.lookup(None, collection, index, equal, after, limit)
    }

    /// One page of the declared index `index` of `collection`, which runs
    /// in one space, in `space`: the space id starts the equality prefix,
    /// and `equal` holds the values of the declared equality prefix after
    /// it (RFC 0006, Amendment 6).
    pub fn page_in(
        &self,
        space: SpaceId,
        collection: &str,
        index: &str,
        equal: &[Value],
        after: Option<&Cursor>,
        limit: usize,
    ) -> Result<Page, Error> {
        self.lookup(Some(space), collection, index, equal, after, limit)
    }

    fn lookup(
        &self,
        space: Option<SpaceId>,
        collection: &str,
        index: &str,
        equal: &[Value],
        after: Option<&Cursor>,
        limit: usize,
    ) -> Result<Page, Error> {
        let index = self
            .model
            .index(collection, index)
            .ok_or(Error::Query(QueryError::UnknownIndex))?;
        let space = match (index.scope, space) {
            (Scope::Member, None) => None,
            (Scope::Space, Some(space)) => Some(Sql::Blob(space.0.to_vec())),
            _ => return Err(Error::Query(QueryError::Space)),
        };
        if equal.len() != index.equality {
            return Err(Error::Query(QueryError::Equality));
        }
        if limit == 0 || limit > View::MAX_PAGE {
            return Err(Error::Query(QueryError::PageSize));
        }
        let equal: Vec<Sql> = space
            .into_iter()
            .chain(equal.iter().cloned().map(sql_value))
            .collect();
        let hash = self.model.hash();
        let fixed = prefix(index);
        let after = match after {
            None => None,
            Some(cursor) => {
                let fits = cursor.collection == index.collection
                    && cursor.index == index.name
                    && cursor.model == hash
                    && cursor.key[..fixed] == equal[..];
                if !fits {
                    return Err(Error::Query(QueryError::Cursor));
                }
                Some(&cursor.key[fixed..])
            }
        };
        let (sql, parameters) = statement(index, &equal, after, limit);
        let mut statement = self.conn.prepare(&sql)?;
        let columns = index_columns(index);
        let width = columns.len();
        let at = |name: &str| columns.iter().position(|(c, _)| c == name);
        let (space_at, object_at) = (at("space_id"), at("object_id"));
        let keys: Vec<Vec<Sql>> = statement
            .query_map(params_from_iter(parameters), |row| {
                (0..width).map(|i| row.get::<_, Sql>(i)).collect()
            })?
            .collect::<Result<_, _>>()?;
        let mut rows = Vec::with_capacity(keys.len());
        for key in &keys {
            let id = |at: Option<usize>| match at.map(|i| &key[i]) {
                Some(Sql::Blob(bytes)) => <[u8; 16]>::try_from(bytes.as_slice()).ok(),
                _ => None,
            };
            let space = id(space_at).ok_or(Error::Corrupt("space id"))?;
            let object = id(object_at).ok_or(Error::Corrupt("object id"))?;
            rows.push((SpaceId(space), ObjectId(object)));
        }
        let next = match keys.last() {
            Some(key) if keys.len() == limit => Some(Cursor {
                collection: index.collection.clone(),
                index: index.name.clone(),
                model: hash,
                key: key.clone(),
            }),
            _ => None,
        };
        Ok(Page { rows, next })
    }

    /// The query plans of the declared index `index` of `collection`: its
    /// first page and a page after a cursor, as SQLite's `EXPLAIN QUERY
    /// PLAN` gives them.
    pub fn plans(&self, collection: &str, index: &str) -> Result<Vec<String>, Error> {
        let index = self
            .model
            .index(collection, index)
            .ok_or(Error::Query(QueryError::UnknownIndex))?;
        plans(&self.conn, index)
    }
}

#[cfg(test)]
mod tests {
    // On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test]
    // functions; this routes #[test] there (NOC-10). The view is native
    // only, so this module is too.
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::fits;

    const IX: &str = "ix_task_by_rank";

    #[test]
    fn a_plan_fits_when_it_walks_only_the_declared_index_without_a_sort() {
        assert!(fits("SCAN c_task USING COVERING INDEX ix_task_by_rank", IX));
        assert!(fits(
            "SEARCH c_task USING INDEX ix_task_by_rank (f_rank>?)",
            IX
        ));
        assert!(!fits(
            "SCAN c_task USING COVERING INDEX ix_task_by_rank; USE TEMP B-TREE FOR ORDER BY",
            IX
        ));
        assert!(!fits("SCAN c_task", IX));
        assert!(!fits(
            "MULTI-INDEX OR; INDEX 1; SEARCH c_task USING INDEX ix_task_by_rank (f_rank>?); \
             INDEX 2; SEARCH c_task USING INDEX ix_task_by_title (f_title=?)",
            IX
        ));
        // A name that only starts like the declared index is another index.
        assert!(!fits("SCAN c_task USING INDEX ix_task_by_rank_2", IX));
    }
}

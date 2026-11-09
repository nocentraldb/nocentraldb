//! One lookup through a declared index (RFC 0006, "Declared indexes and
//! ordered pages"): the rows of the index with these values for its
//! equality prefix, after this cursor, at most N, in index order. The core
//! builds the SQL from the model; there is no raw SQL.
//!
//! The cursor is the last row's index key. The next page is the rows whose
//! key comes after it in the index's order, written out key by key so that
//! an unset value and a descending key compare as the index orders them
//! (SQLite's row values compare all keys one way, and with an unset value
//! they compare to nothing). Every index ends with `(space_id, object_id)`,
//! so the order is total and a page boundary is exact.

use std::fmt;

use rusqlite::types::Value as Sql;
use rusqlite::{Connection, params_from_iter};

use super::fold::sql_value;
use super::model::{Direction, Index};
use super::schema::{column, index_name, rows};
use super::{Error, View};
use crate::cbor::Value;
use crate::ids::{ObjectId, SpaceId};

/// Why a lookup was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryError {
    /// The model declares no index of this name.
    UnknownIndex,
    /// The lookup does not give one value per key of the index's equality
    /// prefix.
    Equality,
    /// The page size is 0 or above [`View::MAX_PAGE`].
    PageSize,
    /// The cursor belongs to another index, another equality prefix or
    /// another model: start again from the first page.
    Cursor,
}

/// The position after the last row of a page: the row's index key, with
/// the index's name and the model's hash. It holds plaintext field values,
/// so its `Debug` output shows only the index and the number of values.
#[derive(Clone, PartialEq)]
pub struct Cursor {
    index: String,
    model: [u8; 32],
    /// The values of every key of the index, then the space id and the
    /// object id.
    key: Vec<Sql>,
}

impl fmt::Debug for Cursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cursor")
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

/// The keys after the equality prefix, then the space and object ids, each
/// with its direction.
fn tail(index: &Index) -> Vec<(String, Direction)> {
    index.keys[index.equality..]
        .iter()
        .map(|(field, direction)| (column(field), *direction))
        .chain(["space_id", "object_id"].map(|c| (c.to_owned(), Direction::Ascending)))
        .collect()
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

/// The SQL of a page, and its parameters: the equality values, the
/// cursor's values that are set, and the page size.
fn statement(
    index: &Index,
    equal: &[Sql],
    after: Option<&[Sql]>,
    limit: usize,
) -> (String, Vec<Sql>) {
    let tail = tail(index);
    let mut parameters: Vec<Sql> = equal.to_vec();
    let mut conditions: Vec<String> = index.keys[..index.equality]
        .iter()
        .map(|(field, _)| format!("{} IS ?", column(field)))
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
    let order: Vec<String> = index
        .keys
        .iter()
        .map(|(field, direction)| (column(field), *direction))
        .chain(["space_id", "object_id"].map(|c| (c.to_owned(), Direction::Ascending)))
        .map(|(name, direction)| match direction {
            Direction::Ascending => format!("{name} ASC"),
            Direction::Descending => format!("{name} DESC"),
        })
        .collect();
    let keys: Vec<String> = index.keys.iter().map(|(field, _)| column(field)).collect();
    let filter = match conditions.is_empty() {
        true => String::new(),
        false => format!(" WHERE {}", conditions.join(" AND ")),
    };
    let sql = format!(
        "SELECT {}space_id, object_id FROM {}{filter} ORDER BY {} LIMIT ?",
        keys.iter().map(|k| format!("{k}, ")).collect::<String>(),
        rows(&index.collection),
        order.join(", "),
    );
    parameters.push(Sql::Integer(i64::try_from(limit).unwrap_or(i64::MAX)));
    (sql, parameters)
}

/// The query plans of an index's first page and of a page after a
/// cursor, one line each.
pub(super) fn plans(conn: &Connection, index: &Index) -> Result<Vec<String>, Error> {
    let equal = vec![Sql::Null; index.equality];
    let after = vec![Sql::Integer(0); index.keys.len() - index.equality + 2];
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
        let uses = format!("INDEX {}", index_name(index));
        for plan in plans(conn, index)? {
            if !plan.contains(&uses) || plan.contains("TEMP B-TREE") {
                return Err(Error::Plan(index.name.clone()));
            }
        }
    }
    Ok(())
}

impl View {
    /// The page size a caller that names none should use.
    pub const DEFAULT_PAGE: usize = 50;
    /// The largest page, so that one read holds the database briefly.
    pub const MAX_PAGE: usize = 500;

    /// One page of the declared index `index`: the rows whose equality
    /// prefix holds `equal`, after `after`, at most `limit`.
    pub fn page(
        &self,
        index: &str,
        equal: &[Value],
        after: Option<&Cursor>,
        limit: usize,
    ) -> Result<Page, Error> {
        let index = self
            .model
            .index(index)
            .ok_or(Error::Query(QueryError::UnknownIndex))?;
        if equal.len() != index.equality {
            return Err(Error::Query(QueryError::Equality));
        }
        if limit == 0 || limit > View::MAX_PAGE {
            return Err(Error::Query(QueryError::PageSize));
        }
        let equal: Vec<Sql> = equal.iter().cloned().map(sql_value).collect();
        let hash = self.model.hash();
        let after = match after {
            None => None,
            Some(cursor) => {
                let fits = cursor.index == index.name
                    && cursor.model == hash
                    && cursor.key[..index.equality] == equal[..];
                if !fits {
                    return Err(Error::Query(QueryError::Cursor));
                }
                Some(&cursor.key[index.equality..])
            }
        };
        let (sql, parameters) = statement(index, &equal, after, limit);
        let mut statement = self.conn.prepare(&sql)?;
        let width = index.keys.len() + 2;
        let keys: Vec<Vec<Sql>> = statement
            .query_map(params_from_iter(parameters), |row| {
                (0..width).map(|i| row.get::<_, Sql>(i)).collect()
            })?
            .collect::<Result<_, _>>()?;
        let mut rows = Vec::with_capacity(keys.len());
        for key in &keys {
            let id = |value: &Sql| match value {
                Sql::Blob(bytes) => <[u8; 16]>::try_from(bytes.as_slice()).ok(),
                _ => None,
            };
            let space = id(&key[width - 2]).ok_or(Error::Corrupt("space id"))?;
            let object = id(&key[width - 1]).ok_or(Error::Corrupt("object id"))?;
            rows.push((SpaceId(space), ObjectId(object)));
        }
        let next = match keys.last() {
            Some(key) if keys.len() == limit => Some(Cursor {
                index: index.name.clone(),
                model: hash,
                key: key.clone(),
            }),
            _ => None,
        };
        Ok(Page { rows, next })
    }

    /// The query plans of the declared index `index`: its first page and a
    /// page after a cursor, as SQLite's `EXPLAIN QUERY PLAN` gives them.
    pub fn plans(&self, index: &str) -> Result<Vec<String>, Error> {
        let index = self
            .model
            .index(index)
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

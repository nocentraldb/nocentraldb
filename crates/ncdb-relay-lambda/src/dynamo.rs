//! The `connections` table on DynamoDB (RFC 0007, "Tables and keys"):
//! one item per open connection and one per subscription, a TTL that
//! deletes what a missed `$disconnect` leaves behind, an index on space id
//! that finds a space's subscribers, and an index on member id that finds
//! a member's connections, to send `added`.
//!
//! Items, under the partition key `connection` (the API Gateway connection
//! id, text) and the sort key `item` (text):
//!
//! - the connection, `item = "#"`: `stage` (`hello`, `auth` or `member`),
//!   `nonce` (bytes, while it waits for `auth`), `member` (bytes, once it
//!   acts as a member; the key of the index `by_member`), `connected_at`
//!   and `expires_at` (milliseconds), `ttl` (seconds, the TTL attribute);
//! - a subscription, `item` = the space id in lowercase hex: `space`
//!   (bytes; the key of the index `by_space`), `subscriber` (bytes, its
//!   member), `req`, `epoch` (bytes), `expires_at` and `ttl` of its
//!   connection.
//!
//! A subscription carries its member as `subscriber`, not `member`, so it
//! never lists in `by_member`. The indexes are read eventually
//! consistent, as DynamoDB serves them: a subscription made a moment ago
//! may miss one live delivery, which RFC 0007 allows ("Delivery to
//! subscribers": best-effort; the member pulls the gap).

use std::collections::HashMap;
use std::time::Duration;

use aws_sdk_dynamodb::Client;
use aws_sdk_dynamodb::primitives::Blob;
use aws_sdk_dynamodb::types::{
    AttributeDefinition, AttributeValue, BillingMode, DeleteRequest, GlobalSecondaryIndex,
    KeySchemaElement, KeyType, Projection, ProjectionType, ScalarAttributeType, TableStatus,
    TimeToLiveSpecification, WriteRequest,
};
use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_relay::store::{Epoch, StoreError};
use tokio::runtime::Handle;

use crate::connections::{Connection, Connections, Stage, Subscription};

/// The sort key of a connection's own item.
const CONNECTION: &str = "#";

/// The index on space id, of subscriptions.
pub const BY_SPACE: &str = "by_space";

/// The index on member id, of connections that act as a member.
pub const BY_MEMBER: &str = "by_member";

type Item = HashMap<String, AttributeValue>;

/// The `connections` table, through a DynamoDB client whose calls run on
/// `runtime`, as the relay's AWS backend runs its own.
#[derive(Clone)]
pub struct DynamoConnections {
    db: Client,
    table: String,
    runtime: Handle,
}

impl std::fmt::Debug for DynamoConnections {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamoConnections")
            .field("table", &self.table)
            .finish_non_exhaustive()
    }
}

fn failed<E: std::fmt::Debug>(what: &'static str) -> impl Fn(E) -> StoreError {
    move |e| StoreError(format!("the connections table: {what}: {e:?}"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn s(text: impl Into<String>) -> AttributeValue {
    AttributeValue::S(text.into())
}

fn b(bytes: &[u8]) -> AttributeValue {
    AttributeValue::B(Blob::new(bytes))
}

fn n(number: u64) -> AttributeValue {
    AttributeValue::N(number.to_string())
}

fn number(item: &Item, name: &str) -> Option<u64> {
    item.get(name)?.as_n().ok()?.parse().ok()
}

fn bytes<const N: usize>(item: &Item, name: &str) -> Option<[u8; N]> {
    item.get(name)?.as_b().ok()?.as_ref().try_into().ok()
}

fn text<'a>(item: &'a Item, name: &str) -> Option<&'a str> {
    item.get(name)?.as_s().ok().map(String::as_str)
}

fn malformed() -> StoreError {
    StoreError("the connections table holds an item it cannot read".into())
}

/// A connection's item, as read back.
fn connection(item: &Item) -> Result<Connection, StoreError> {
    let stage = match text(item, "stage") {
        Some("hello") => Stage::Hello,
        Some("auth") => Stage::Auth {
            nonce: bytes(item, "nonce").ok_or_else(malformed)?,
        },
        Some("member") => Stage::Member(MemberId(bytes(item, "member").ok_or_else(malformed)?)),
        _ => return Err(malformed()),
    };
    Ok(Connection {
        stage,
        connected_at: number(item, "connected_at").ok_or_else(malformed)?,
        expires_at: number(item, "expires_at").ok_or_else(malformed)?,
    })
}

fn subscription(item: &Item) -> Result<(String, Subscription), StoreError> {
    let id = text(item, "connection").ok_or_else(malformed)?.to_owned();
    Ok((
        id,
        Subscription {
            member: MemberId(bytes(item, "subscriber").ok_or_else(malformed)?),
            req: number(item, "req").ok_or_else(malformed)?,
            epoch: Epoch(bytes(item, "epoch").ok_or_else(malformed)?),
        },
    ))
}

impl DynamoConnections {
    /// The table `table`, through `db`, whose calls run on `runtime`.
    pub fn new(db: Client, table: impl Into<String>, runtime: Handle) -> DynamoConnections {
        DynamoConnections {
            db,
            table: table.into(),
            runtime,
        }
    }

    /// Runs `call` on the store's runtime once the returned future is
    /// polled, on whatever executor polls it.
    fn run<T, F, C>(
        &self,
        call: C,
    ) -> impl Future<Output = Result<T, StoreError>> + Send + use<T, F, C>
    where
        T: Send + 'static,
        F: Future<Output = Result<T, StoreError>> + Send + 'static,
        C: FnOnce(Client, String) -> F + Send + 'static,
    {
        let (db, table, runtime) = (self.db.clone(), self.table.clone(), self.runtime.clone());
        async move {
            match runtime.spawn(call(db, table)).await {
                Ok(result) => result,
                Err(_) => Err(StoreError("a call of the connections table stopped".into())),
            }
        }
    }
}

/// Every item of the connection `id`: its own and its subscriptions'.
async fn items_of(db: &Client, table: &str, id: &str) -> Result<Vec<Item>, StoreError> {
    let mut items = Vec::new();
    let mut start = None;
    loop {
        let out = db
            .query()
            .table_name(table)
            .key_condition_expression("#c = :c")
            .expression_attribute_names("#c", "connection")
            .expression_attribute_values(":c", s(id))
            .consistent_read(true)
            .set_exclusive_start_key(start)
            .send()
            .await
            .map_err(failed("read a connection's items"))?;
        items.extend(out.items.unwrap_or_default());
        start = out.last_evaluated_key;
        if start.is_none() {
            return Ok(items);
        }
    }
}

/// The items of an index under `value` of `key`.
async fn index(
    db: &Client,
    table: &str,
    name: &str,
    key: &str,
    value: &[u8],
) -> Result<Vec<Item>, StoreError> {
    let mut items = Vec::new();
    let mut start = None;
    loop {
        let out = db
            .query()
            .table_name(table)
            .index_name(name)
            .key_condition_expression("#k = :k")
            .expression_attribute_names("#k", key)
            .expression_attribute_values(":k", b(value))
            .set_exclusive_start_key(start)
            .send()
            .await
            .map_err(failed("read an index"))?;
        items.extend(out.items.unwrap_or_default());
        start = out.last_evaluated_key;
        if start.is_none() {
            return Ok(items);
        }
    }
}

/// Whether an item's record is still live at `now`.
fn live(item: &Item, now: u64) -> bool {
    number(item, "expires_at").is_some_and(|at| at > now)
}

impl Connections for DynamoConnections {
    fn put(
        &self,
        id: &str,
        connection: Connection,
    ) -> impl Future<Output = Result<(), StoreError>> + Send {
        let id = id.to_owned();
        self.run(move |db, table| async move {
            let mut put = db
                .put_item()
                .table_name(table)
                .item("connection", s(id))
                .item("item", s(CONNECTION))
                .item("connected_at", n(connection.connected_at))
                .item("expires_at", n(connection.expires_at))
                .item("ttl", n(connection.expires_at / 1000));
            put = match connection.stage {
                Stage::Hello => put.item("stage", s("hello")),
                Stage::Auth { nonce } => put.item("stage", s("auth")).item("nonce", b(&nonce)),
                Stage::Member(member) => {
                    put.item("stage", s("member")).item("member", b(&member.0))
                }
            };
            put.send().await.map_err(failed("store a connection"))?;
            Ok(())
        })
    }

    fn get(&self, id: &str) -> impl Future<Output = Result<Option<Connection>, StoreError>> + Send {
        let id = id.to_owned();
        self.run(move |db, table| async move {
            let out = db
                .get_item()
                .table_name(table)
                .key("connection", s(id))
                .key("item", s(CONNECTION))
                .consistent_read(true)
                .send()
                .await
                .map_err(failed("read a connection"))?;
            match out.item {
                Some(item) if live(&item, crate::contract::now()) => connection(&item).map(Some),
                _ => Ok(None),
            }
        })
    }

    fn remove(&self, id: &str) -> impl Future<Output = Result<(), StoreError>> + Send {
        let id = id.to_owned();
        self.run(move |db, table| async move {
            let items = items_of(&db, &table, &id).await?;
            // BatchWriteItem takes at most 25 requests.
            for chunk in items.chunks(25) {
                let mut requests = Vec::with_capacity(chunk.len());
                for item in chunk {
                    let key: Item = ["connection", "item"]
                        .into_iter()
                        .filter_map(|k| Some((k.to_owned(), item.get(k)?.clone())))
                        .collect();
                    let delete = DeleteRequest::builder()
                        .set_key(Some(key))
                        .build()
                        .map_err(failed("build a delete"))?;
                    requests.push(WriteRequest::builder().delete_request(delete).build());
                }
                let mut pending = HashMap::from([(table.clone(), requests)]);
                for _ in 0..8 {
                    let out = db
                        .batch_write_item()
                        .set_request_items(Some(pending))
                        .send()
                        .await
                        .map_err(failed("remove a connection"))?;
                    pending = out.unprocessed_items.unwrap_or_default();
                    if pending.values().all(Vec::is_empty) {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                if !pending.values().all(Vec::is_empty) {
                    return Err(StoreError(
                        "the connections table: a remove stayed unprocessed".into(),
                    ));
                }
            }
            Ok(())
        })
    }

    fn subscribe(
        &self,
        id: &str,
        space: SpaceId,
        subscription: Subscription,
    ) -> impl Future<Output = Result<(), StoreError>> + Send {
        let id = id.to_owned();
        self.run(move |db, table| async move {
            // A subscription expires with its connection.
            let own = db
                .get_item()
                .table_name(&table)
                .key("connection", s(&id))
                .key("item", s(CONNECTION))
                .consistent_read(true)
                .send()
                .await
                .map_err(failed("read a connection"))?;
            let expires_at = own
                .item
                .as_ref()
                .and_then(|item| number(item, "expires_at"))
                .unwrap_or(0);
            db.put_item()
                .table_name(table)
                .item("connection", s(id))
                .item("item", s(hex(&space.0)))
                .item("space", b(&space.0))
                .item("subscriber", b(&subscription.member.0))
                .item("req", n(subscription.req))
                .item("epoch", b(&subscription.epoch.0))
                .item("expires_at", n(expires_at))
                .item("ttl", n(expires_at / 1000))
                .send()
                .await
                .map_err(failed("store a subscription"))?;
            Ok(())
        })
    }

    fn unsubscribe(
        &self,
        id: &str,
        space: SpaceId,
    ) -> impl Future<Output = Result<(), StoreError>> + Send {
        let id = id.to_owned();
        self.run(move |db, table| async move {
            db.delete_item()
                .table_name(table)
                .key("connection", s(id))
                .key("item", s(hex(&space.0)))
                .send()
                .await
                .map_err(failed("end a subscription"))?;
            Ok(())
        })
    }

    fn subscribers(
        &self,
        space: SpaceId,
    ) -> impl Future<Output = Result<Vec<(String, Subscription)>, StoreError>> + Send {
        self.run(move |db, table| async move {
            let now = crate::contract::now();
            let items = index(&db, &table, BY_SPACE, "space", &space.0).await?;
            let mut found = items
                .iter()
                .filter(|item| live(item, now))
                .map(subscription)
                .collect::<Result<Vec<_>, _>>()?;
            found.sort_by(|a, b| a.0.cmp(&b.0));
            Ok(found)
        })
    }

    fn of_member(
        &self,
        member: MemberId,
    ) -> impl Future<Output = Result<Vec<String>, StoreError>> + Send {
        self.run(move |db, table| async move {
            let now = crate::contract::now();
            let items = index(&db, &table, BY_MEMBER, "member", &member.0).await?;
            let mut found: Vec<String> = items
                .iter()
                .filter(|item| live(item, now))
                .filter_map(|item| text(item, "connection").map(str::to_owned))
                .collect();
            found.sort();
            Ok(found)
        })
    }
}

fn built(e: impl std::fmt::Display) -> StoreError {
    StoreError(format!("the connections table's schema: {e}"))
}

fn key(name: &str, kind: KeyType) -> Result<KeySchemaElement, StoreError> {
    KeySchemaElement::builder()
        .attribute_name(name)
        .key_type(kind)
        .build()
        .map_err(built)
}

fn attribute(name: &str, kind: ScalarAttributeType) -> Result<AttributeDefinition, StoreError> {
    AttributeDefinition::builder()
        .attribute_name(name)
        .attribute_type(kind)
        .build()
        .map_err(built)
}

/// Creates the table `table` with its indexes and TTL, for the tests and
/// a relay on one machine; a deployment makes the same with its own
/// tooling (`infra/aws`). Waits until it is active.
pub async fn create(db: &Client, table: &str) -> Result<(), StoreError> {
    let by_space = GlobalSecondaryIndex::builder()
        .index_name(BY_SPACE)
        .key_schema(key("space", KeyType::Hash)?)
        .projection(
            Projection::builder()
                .projection_type(ProjectionType::All)
                .build(),
        )
        .build()
        .map_err(built)?;
    let by_member = GlobalSecondaryIndex::builder()
        .index_name(BY_MEMBER)
        .key_schema(key("member", KeyType::Hash)?)
        .projection(
            Projection::builder()
                .projection_type(ProjectionType::Include)
                .non_key_attributes("expires_at")
                .build(),
        )
        .build()
        .map_err(built)?;
    db.create_table()
        .table_name(table)
        .billing_mode(BillingMode::PayPerRequest)
        .attribute_definitions(attribute("connection", ScalarAttributeType::S)?)
        .attribute_definitions(attribute("item", ScalarAttributeType::S)?)
        .attribute_definitions(attribute("space", ScalarAttributeType::B)?)
        .attribute_definitions(attribute("member", ScalarAttributeType::B)?)
        .key_schema(key("connection", KeyType::Hash)?)
        .key_schema(key("item", KeyType::Range)?)
        .global_secondary_indexes(by_space)
        .global_secondary_indexes(by_member)
        .send()
        .await
        .map_err(failed("create the table"))?;
    for _ in 0..120 {
        let out = db
            .describe_table()
            .table_name(table)
            .send()
            .await
            .map_err(failed("describe the table"))?;
        if let Some(description) = out.table {
            let indexes = description
                .global_secondary_indexes()
                .iter()
                .all(|index| index.index_status().is_none_or(|s| s.as_str() == "ACTIVE"));
            if description.table_status == Some(TableStatus::Active) && indexes {
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    db.update_time_to_live()
        .table_name(table)
        .time_to_live_specification(
            TimeToLiveSpecification::builder()
                .attribute_name("ttl")
                .enabled(true)
                .build()
                .map_err(built)?,
        )
        .send()
        .await
        .map_err(failed("turn on the TTL"))?;
    Ok(())
}

/// Deletes the table `table`.
pub async fn delete(db: &Client, table: &str) -> Result<(), StoreError> {
    db.delete_table()
        .table_name(table)
        .send()
        .await
        .map_err(failed("delete the table"))?;
    Ok(())
}

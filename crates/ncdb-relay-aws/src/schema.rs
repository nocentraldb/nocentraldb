//! The backend's tables and bucket, as RFC 0007 lays them out ("Tables and
//! keys", "How large changes go to S3"): every table on demand, binary ids
//! and numeric positions, the index of `members` on member id, a TTL on
//! uploads, and the lifecycle rule that deletes uploads after a
//! day. [`create`] makes them, for the tests and for a relay on one
//! machine; a deployment makes the same with its own tooling. [`delete`]
//! removes them with everything in them.

use std::time::Duration;

use aws_sdk_dynamodb::types::{
    AttributeDefinition, BillingMode, GlobalSecondaryIndex, KeySchemaElement, KeyType, Projection,
    ProjectionType, ScalarAttributeType, TableStatus, TimeToLiveSpecification,
};
use aws_sdk_s3::types::{
    BucketLifecycleConfiguration, BucketLocationConstraint, CreateBucketConfiguration,
    ExpirationStatus, LifecycleExpiration, LifecycleRule, LifecycleRuleFilter,
};
use ncdb_relay::store::StoreError;

use crate::Names;
use crate::item::failed;
use crate::keys::BY_MEMBER;

/// A key attribute: its name, whether it is binary or a number, and
/// whether it is the partition or the sort key.
type Key = (&'static str, ScalarAttributeType, KeyType);

fn binary(name: &'static str, key: KeyType) -> Key {
    (name, ScalarAttributeType::B, key)
}

/// The key of each table, by its name in `names`.
fn tables(names: &Names) -> Vec<(&str, Vec<Key>)> {
    use KeyType::{Hash, Range};
    vec![
        (
            &names.log,
            vec![
                binary("space", Hash),
                ("position", ScalarAttributeType::N, Range),
            ],
        ),
        (
            &names.changes,
            vec![binary("space", Hash), binary("change", Range)],
        ),
        (&names.spaces, vec![binary("space", Hash)]),
        (
            &names.members,
            vec![binary("space", Hash), binary("member", Range)],
        ),
        (
            &names.person_keys,
            vec![binary("space", Hash), binary("id", Range)],
        ),
        (&names.uploads, vec![binary("id", Hash)]),
    ]
}

fn built(e: impl std::fmt::Display) -> StoreError {
    StoreError(format!("the schema: {e}"))
}

/// Creates the tables and the bucket, and waits until every table is
/// active.
pub async fn create(
    db: &aws_sdk_dynamodb::Client,
    s3: &aws_sdk_s3::Client,
    names: &Names,
) -> Result<(), StoreError> {
    for (table, keys) in tables(names) {
        let mut create = db
            .create_table()
            .table_name(table)
            .billing_mode(BillingMode::PayPerRequest);
        for (name, kind, key) in &keys {
            create = create
                .attribute_definitions(
                    AttributeDefinition::builder()
                        .attribute_name(*name)
                        .attribute_type(kind.clone())
                        .build()
                        .map_err(built)?,
                )
                .key_schema(
                    KeySchemaElement::builder()
                        .attribute_name(*name)
                        .key_type(key.clone())
                        .build()
                        .map_err(built)?,
                );
        }
        if table == names.members {
            create = create.global_secondary_indexes(by_member()?);
        }
        create.send().await.map_err(failed("create a table"))?;
    }
    for table in tables(names).into_iter().map(|(table, _)| table) {
        active(db, table).await?;
    }
    for table in [&names.uploads] {
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
            .map_err(failed("turn on a TTL"))?;
    }
    bucket(s3, &names.bucket).await
}

/// The index of `members` on member id, sorted by space id, with whether
/// the member is current.
fn by_member() -> Result<GlobalSecondaryIndex, StoreError> {
    GlobalSecondaryIndex::builder()
        .index_name(BY_MEMBER)
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("member")
                .key_type(KeyType::Hash)
                .build()
                .map_err(built)?,
        )
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("space")
                .key_type(KeyType::Range)
                .build()
                .map_err(built)?,
        )
        .projection(
            Projection::builder()
                .projection_type(ProjectionType::Include)
                .non_key_attributes("current")
                .build(),
        )
        .build()
        .map_err(built)
}

/// Waits until a table and its indexes are active, for at most a minute.
async fn active(db: &aws_sdk_dynamodb::Client, table: &str) -> Result<(), StoreError> {
    for _ in 0..120 {
        let out = db
            .describe_table()
            .table_name(table)
            .send()
            .await
            .map_err(failed("describe a table"))?;
        let Some(description) = out.table else {
            continue;
        };
        let indexes_active = description
            .global_secondary_indexes()
            .iter()
            .all(|index| index.index_status().is_none_or(|s| s.as_str() == "ACTIVE"));
        if description.table_status == Some(TableStatus::Active) && indexes_active {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err(StoreError("a table did not become active".into()))
}

/// Creates the bucket, in the S3 client's region, with the rule that
/// deletes uploads a day old.
async fn bucket(s3: &aws_sdk_s3::Client, bucket: &str) -> Result<(), StoreError> {
    let mut create = s3.create_bucket().bucket(bucket);
    let region = s3.config().region().map(|r| r.as_ref().to_owned());
    if let Some(region) = region.filter(|r| r != "us-east-1") {
        create = create.create_bucket_configuration(
            CreateBucketConfiguration::builder()
                .location_constraint(BucketLocationConstraint::from(region.as_str()))
                .build(),
        );
    }
    create.send().await.map_err(failed("create the bucket"))?;
    let rule = LifecycleRule::builder()
        .id("uploads-after-a-day")
        .filter(LifecycleRuleFilter::builder().prefix("uploads/").build())
        .expiration(LifecycleExpiration::builder().days(1).build())
        .status(ExpirationStatus::Enabled)
        .build()
        .map_err(built)?;
    s3.put_bucket_lifecycle_configuration()
        .bucket(bucket)
        .lifecycle_configuration(
            BucketLifecycleConfiguration::builder()
                .rules(rule)
                .build()
                .map_err(built)?,
        )
        .send()
        .await
        .map_err(failed("set the bucket's lifecycle rule"))?;
    Ok(())
}

/// Deletes the tables, and the bucket with every object in it.
pub async fn delete(
    db: &aws_sdk_dynamodb::Client,
    s3: &aws_sdk_s3::Client,
    names: &Names,
) -> Result<(), StoreError> {
    for (table, _) in tables(names) {
        db.delete_table()
            .table_name(table)
            .send()
            .await
            .map_err(failed("delete a table"))?;
    }
    let mut pages = s3
        .list_objects_v2()
        .bucket(&names.bucket)
        .into_paginator()
        .send();
    while let Some(page) = pages.next().await {
        let page = page.map_err(failed("list the bucket"))?;
        for key in page.contents().iter().filter_map(|o| o.key()) {
            s3.delete_object()
                .bucket(&names.bucket)
                .key(key)
                .send()
                .await
                .map_err(failed("empty the bucket"))?;
        }
    }
    s3.delete_bucket()
        .bucket(&names.bucket)
        .send()
        .await
        .map_err(failed("delete the bucket"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backend_creates_six_tables_and_no_tokens_table() {
        // NOC-295 A: behind API Gateway the URLs of large changes are
        // presigned S3 URLs, so the backend keeps no tokens (NOC-303).
        let names = Names::with_prefix("p-");
        let created: Vec<&str> = tables(&names).into_iter().map(|(t, _)| t).collect();
        assert_eq!(
            created,
            [
                "p-log",
                "p-changes",
                "p-spaces",
                "p-members",
                "p-person-keys",
                "p-uploads"
            ]
        );
    }
}

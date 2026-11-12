//! The DynamoDB operations of RFC 0007's tables: a table with a sort key
//! and a global secondary index, TTL, the append's conditional transaction,
//! the pull's range query, the head's conditional update and point-in-time
//! recovery.

use aws_sdk_dynamodb::types::{
    AttributeDefinition, AttributeValue, BillingMode, GlobalSecondaryIndex, KeySchemaElement,
    KeyType, PointInTimeRecoverySpecification, Projection, ProjectionType, Put,
    ScalarAttributeType, TimeToLiveSpecification, TransactWriteItem,
};

use super::common::{Outcome, failed};

fn s(value: &str) -> AttributeValue {
    AttributeValue::S(value.to_owned())
}

fn n(value: u64) -> AttributeValue {
    AttributeValue::N(value.to_string())
}

fn key(name: &str, kind: KeyType) -> KeySchemaElement {
    KeySchemaElement::builder()
        .attribute_name(name)
        .key_type(kind)
        .build()
        .expect("a key schema element")
}

fn attribute(name: &str, kind: ScalarAttributeType) -> AttributeDefinition {
    AttributeDefinition::builder()
        .attribute_name(name)
        .attribute_type(kind)
        .build()
        .expect("an attribute definition")
}

const TABLE: &str = "log";

/// `CreateTable` with a sort key and a global secondary index, as RFC
/// 0007's `members` and `connections` tables have.
pub async fn create_table(db: &aws_sdk_dynamodb::Client) -> Outcome {
    let index = GlobalSecondaryIndex::builder()
        .index_name("byAuthor")
        .key_schema(key("author", KeyType::Hash))
        .projection(
            Projection::builder()
                .projection_type(ProjectionType::All)
                .build(),
        )
        .build()
        .expect("an index");
    let created = db
        .create_table()
        .table_name(TABLE)
        .billing_mode(BillingMode::PayPerRequest)
        .key_schema(key("space", KeyType::Hash))
        .key_schema(key("position", KeyType::Range))
        .attribute_definitions(attribute("space", ScalarAttributeType::S))
        .attribute_definitions(attribute("position", ScalarAttributeType::N))
        .attribute_definitions(attribute("author", ScalarAttributeType::S))
        .global_secondary_indexes(index)
        .send()
        .await;
    if let Err(error) = created {
        return failed(&error);
    }
    match db.describe_table().table_name(TABLE).send().await {
        Ok(out) => {
            let indexes = out
                .table()
                .map(|t| t.global_secondary_indexes().len())
                .unwrap_or_default();
            match indexes {
                1 => Outcome::Supported,
                other => Outcome::Differs(format!("DescribeTable lists {other} indexes, not 1")),
            }
        }
        Err(error) => failed(&error),
    }
}

/// `UpdateTimeToLive`, as on `connections`.
pub async fn time_to_live(db: &aws_sdk_dynamodb::Client) -> Outcome {
    let spec = TimeToLiveSpecification::builder()
        .enabled(true)
        .attribute_name("expiresAt")
        .build()
        .expect("a TTL specification");
    if let Err(error) = db
        .update_time_to_live()
        .table_name(TABLE)
        .time_to_live_specification(spec)
        .send()
        .await
    {
        return failed(&error);
    }
    match db.describe_time_to_live().table_name(TABLE).send().await {
        Ok(out) => match out
            .time_to_live_description()
            .and_then(|d| d.attribute_name())
        {
            Some("expiresAt") => Outcome::Supported,
            other => Outcome::Differs(format!("DescribeTimeToLive names {other:?}")),
        },
        Err(error) => failed(&error),
    }
}

fn put(position: u64) -> TransactWriteItem {
    let put = Put::builder()
        .table_name(TABLE)
        .item("space", s("s1"))
        .item("position", n(position))
        .item("author", s("a1"))
        .condition_expression("attribute_not_exists(#p)")
        .expression_attribute_names("#p", "position")
        .build()
        .expect("a put");
    TransactWriteItem::builder().put(put).build()
}

/// `TransactWriteItems` whose conditions hold: an append of three changes.
pub async fn transaction(db: &aws_sdk_dynamodb::Client) -> Outcome {
    match db
        .transact_write_items()
        .set_transact_items(Some(vec![put(1), put(2), put(3)]))
        .send()
        .await
    {
        Ok(_) => Outcome::Supported,
        Err(error) => failed(&error),
    }
}

/// `TransactWriteItems` whose second condition fails: the transaction is
/// cancelled, with `ConditionalCheckFailed` as the second reason and none
/// for the others.
pub async fn failed_transaction(db: &aws_sdk_dynamodb::Client) -> Outcome {
    let result = db
        .transact_write_items()
        .set_transact_items(Some(vec![put(4), put(2)]))
        .send()
        .await;
    let Err(error) = result else {
        return Outcome::Differs("the transaction succeeded".into());
    };
    let Some(service) = error.as_service_error() else {
        return failed(&error);
    };
    let aws_sdk_dynamodb::operation::transact_write_items::TransactWriteItemsError::TransactionCanceledException(
        cancelled,
    ) = service
    else {
        return failed(&error);
    };
    let codes: Vec<&str> = cancelled
        .cancellation_reasons()
        .iter()
        .map(|r| r.code().unwrap_or("None"))
        .collect();
    match codes.as_slice() {
        ["None", "ConditionalCheckFailed"] => {}
        other => return Outcome::Differs(format!("cancellation reasons {other:?}")),
    }
    // Nothing of the cancelled transaction was written.
    match db
        .get_item()
        .table_name(TABLE)
        .key("space", s("s1"))
        .key("position", n(4))
        .send()
        .await
    {
        Ok(out) if out.item().is_none() => Outcome::Supported,
        Ok(_) => Outcome::Differs("a cancelled transaction left an item".into()),
        Err(error) => failed(&error),
    }
}

/// `Query` with a sort-key range, as a pull: `position > 1`, in order.
pub async fn query(db: &aws_sdk_dynamodb::Client) -> Outcome {
    match db
        .query()
        .table_name(TABLE)
        .key_condition_expression("#s = :s AND #p > :after")
        .expression_attribute_names("#s", "space")
        .expression_attribute_names("#p", "position")
        .expression_attribute_values(":s", s("s1"))
        .expression_attribute_values(":after", n(1))
        .send()
        .await
    {
        Ok(out) => {
            let positions: Vec<String> = out
                .items()
                .iter()
                .filter_map(|i| i.get("position")?.as_n().ok().cloned())
                .collect();
            match positions.as_slice() {
                [a, b] if a == "2" && b == "3" => Outcome::Supported,
                other => Outcome::Differs(format!("positions {other:?}, not [2, 3]")),
            }
        }
        Err(error) => failed(&error),
    }
}

/// `Query` on the global secondary index.
pub async fn query_index(db: &aws_sdk_dynamodb::Client) -> Outcome {
    match db
        .query()
        .table_name(TABLE)
        .index_name("byAuthor")
        .key_condition_expression("author = :a")
        .expression_attribute_values(":a", s("a1"))
        .send()
        .await
    {
        Ok(out) if out.count() == 3 => Outcome::Supported,
        Ok(out) => Outcome::Differs(format!("{} items, not 3", out.count())),
        Err(error) => failed(&error),
    }
}

/// `UpdateItem` with a condition, as the head of a space: one that holds,
/// then one that fails with `ConditionalCheckFailedException`.
pub async fn conditional_update(db: &aws_sdk_dynamodb::Client) -> Outcome {
    let update = |expected: u64| {
        db.update_item()
            .table_name(TABLE)
            .key("space", s("s1"))
            .key("position", n(3))
            .update_expression("SET head = :next")
            .condition_expression("attribute_not_exists(head) OR head = :expected")
            .expression_attribute_values(":next", n(expected + 1))
            .expression_attribute_values(":expected", n(expected))
            .send()
    };
    if let Err(error) = update(0).await {
        return failed(&error);
    }
    match update(0).await {
        Err(error)
            if error
                .as_service_error()
                .is_some_and(|e| e.is_conditional_check_failed_exception()) =>
        {
            Outcome::Supported
        }
        Err(error) => failed(&error),
        Ok(_) => Outcome::Differs("a failing condition was written".into()),
    }
}

/// `UpdateContinuousBackups`: point-in-time recovery on.
pub async fn point_in_time_recovery(db: &aws_sdk_dynamodb::Client) -> Outcome {
    let spec = PointInTimeRecoverySpecification::builder()
        .point_in_time_recovery_enabled(true)
        .build()
        .expect("a PITR specification");
    if let Err(error) = db
        .update_continuous_backups()
        .table_name(TABLE)
        .point_in_time_recovery_specification(spec)
        .send()
        .await
    {
        return failed(&error);
    }
    match db
        .describe_continuous_backups()
        .table_name(TABLE)
        .send()
        .await
    {
        Ok(out) => {
            let status = out
                .continuous_backups_description()
                .and_then(|d| d.point_in_time_recovery_description())
                .and_then(|p| p.point_in_time_recovery_status())
                .map(|s| s.as_str().to_owned());
            match status.as_deref() {
                Some("ENABLED") => Outcome::Supported,
                other => Outcome::Differs(format!("status {other:?}")),
            }
        }
        Err(error) => failed(&error),
    }
}

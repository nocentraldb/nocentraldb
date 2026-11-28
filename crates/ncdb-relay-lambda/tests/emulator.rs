//! Against the AWS Emulator (docs/aws-emulator.md): the `connections` table
//! on DynamoDB keeps the same contract as the in-memory store, keeps an
//! ended connection's items for the days set and by default none, and the
//! delivery to a gone subscriber is marked as waiting for NOC-278. The tests
//! are ignored unless asked for, and refuse to run against anything but an
//! emulator on this machine.

use std::collections::HashMap;

use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::types::AttributeValue;
use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_relay::store::Epoch;
use ncdb_relay_lambda::apigw::ApiGatewayApi;
use ncdb_relay_lambda::connections::{
    Connection, Connections, MAX_CONNECTION_AGE, Stage, Subscription,
};
use ncdb_relay_lambda::dynamo::{self, DynamoConnections};
use ncdb_relay_lambda::{ConnectionApi, Posted};

/// A connection id of API Gateway's form that no `$connect` ever issued.
const NEVER_ISSUED: &str = "L0SM9cOFvHcCIhw=";

/// Refuses real credentials and any endpoint but a loopback address.
fn emulator_only() {
    for name in [
        "AWS_ACCESS_KEY_ID",
        "AWS_SECRET_ACCESS_KEY",
        "AWS_SESSION_TOKEN",
        "AWS_PROFILE",
    ] {
        assert!(
            std::env::var_os(name).is_none(),
            "{name} is set: unset real credentials before running against the emulator"
        );
    }
    let url = std::env::var("AWS_ENDPOINT_URL")
        .expect("AWS_ENDPOINT_URL names the emulator, as docs/aws-emulator.md says");
    let host = url
        .trim_start_matches("https://")
        .split([':', '/'])
        .next()
        .unwrap_or_default();
    assert!(
        matches!(host, "127.0.0.1" | "localhost"),
        "AWS_ENDPOINT_URL is not a loopback address: {url}"
    );
}

/// Runs `test` against a fresh `connections` table on the emulator, and
/// deletes the table afterwards, also when `test` panics.
fn with_table(test: impl FnOnce(&tokio::runtime::Runtime, &aws_sdk_dynamodb::Client, &str)) {
    emulator_only();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let config = runtime.block_on(aws_config::defaults(BehaviorVersion::latest()).load());
    let db = aws_sdk_dynamodb::Client::new(&config);
    let mut suffix = [0u8; 6];
    getrandom::fill(&mut suffix).expect("a secure generator");
    let suffix: String = suffix.iter().map(|b| format!("{b:02x}")).collect();
    let table = format!("ncdb-{suffix}-connections");
    runtime
        .block_on(dynamo::create(&db, &table))
        .expect("the table");
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        test(&runtime, &db, &table);
    }));
    let _ = runtime.block_on(dynamo::delete(&db, &table));
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}

#[test]
#[ignore = "needs the AWS Emulator (docs/aws-emulator.md)"]
fn the_dynamodb_store_keeps_the_connections_contract() {
    with_table(|runtime, db, table| {
        // The contract's futures run on any executor; the store's calls run
        // on the runtime it was given.
        let store = DynamoConnections::new(db.clone(), table, runtime.handle().clone());
        runtime.block_on(ncdb_relay_lambda::contract::run(&store, "dynamo"));
        let kept = store.keeping(30);
        runtime.block_on(ncdb_relay_lambda::contract::run(&kept, "kept"));
    });
}

const DAY: u64 = 24 * 60 * 60 * 1000;

/// The items of the connection `id`, as the table holds them.
fn items(
    runtime: &tokio::runtime::Runtime,
    db: &aws_sdk_dynamodb::Client,
    table: &str,
    id: &str,
) -> Vec<HashMap<String, AttributeValue>> {
    let out = runtime
        .block_on(
            db.query()
                .table_name(table)
                .key_condition_expression("#c = :c")
                .expression_attribute_names("#c", "connection")
                .expression_attribute_values(":c", AttributeValue::S(id.to_owned()))
                .consistent_read(true)
                .send(),
        )
        .expect("the connection's items");
    out.items.unwrap_or_default()
}

fn number(item: &HashMap<String, AttributeValue>, name: &str) -> u64 {
    item[name]
        .as_n()
        .expect("a number")
        .parse()
        .expect("a number")
}

/// Opens a connection that acts as a member, subscribes it to two spaces,
/// ends one subscription, then the connection.
fn open_subscribe_and_end(runtime: &tokio::runtime::Runtime, store: &DynamoConnections) {
    let member = MemberId([0x11; 32]);
    let now = ncdb_relay_lambda::contract::now();
    let connection = Connection {
        stage: Stage::Member(member),
        connected_at: now,
        expires_at: now + MAX_CONNECTION_AGE,
    };
    let sub = Subscription {
        member,
        req: 1,
        epoch: Epoch([0x55; 16]),
    };
    runtime.block_on(async {
        store.put("c", connection).await.unwrap();
        store
            .subscribe("c", SpaceId([0x33; 16]), sub)
            .await
            .unwrap();
        store
            .subscribe("c", SpaceId([0x44; 16]), sub)
            .await
            .unwrap();
        store.unsubscribe("c", SpaceId([0x44; 16])).await.unwrap();
        store.remove("c").await.unwrap();
    });
}

#[test]
#[ignore = "needs the AWS Emulator (docs/aws-emulator.md)"]
fn the_dynamodb_store_keeps_ended_records_for_the_days_set() {
    with_table(|runtime, db, table| {
        let store = DynamoConnections::new(db.clone(), table, runtime.handle().clone()).keeping(30);
        open_subscribe_and_end(runtime, &store);
        let kept = items(runtime, db, table, "c");
        // The connection and both its subscriptions.
        assert_eq!(kept.len(), 3, "{kept:?}");
        for item in &kept {
            let ended = number(item, "ended_at");
            assert!(number(item, "expires_at") <= ended, "{item:?}");
            assert_eq!(number(item, "ttl"), (ended + 30 * DAY) / 1000, "{item:?}");
        }
        let own = kept
            .iter()
            .find(|item| item["item"].as_s().is_ok_and(|s| s == "#"))
            .expect("the connection's own item");
        assert_eq!(own["member"].as_b().unwrap().as_ref(), [0x11; 32]);
    });
}

#[test]
#[ignore = "needs the AWS Emulator (docs/aws-emulator.md)"]
fn by_default_the_dynamodb_store_keeps_nothing_of_an_ended_connection() {
    with_table(|runtime, db, table| {
        let store = DynamoConnections::new(db.clone(), table, runtime.handle().clone());
        open_subscribe_and_end(runtime, &store);
        assert!(items(runtime, db, table, "c").is_empty());
    });
}

/// NOC-278: the relay forgets a subscriber when `@connections` answers
/// `GoneException` (410; RFC 0007, "Delivery to subscribers"). The emulator
/// cannot show that yet: it serves no WebSocket data plane, so it never
/// issues a connection id, and it answers an id it never issued with 400
/// `BadRequestException`, not AWS's 410. Until it does, delivery to a gone
/// subscriber is tested against a local endpoint that answers as AWS does
/// (`tests/apigw.rs`) and through the handler (`tests/handler.rs`), each
/// marked as standing in for this test.
///
/// This test pins the gap, so it is never silently skipped: it fails the day
/// the emulator answers 410, and that day it becomes the delivery test
/// against the emulator.
#[test]
#[ignore = "needs the AWS Emulator (docs/aws-emulator.md)"]
fn delivery_to_a_gone_subscriber_waits_for_noc_278() {
    emulator_only();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let config = runtime.block_on(aws_config::defaults(BehaviorVersion::latest()).load());
    let api = ApiGatewayApi::new(
        aws_sdk_apigatewaymanagement::Client::new(&config),
        runtime.handle().clone(),
    );
    let closed = "NOC-278 is closed: the emulator answers GoneException. \
                  Make this the delivery test against it (a post reports Posted::Gone, \
                  a close is done) and drop the stand-in marks in tests/apigw.rs and \
                  tests/handler.rs";
    match runtime.block_on(api.post(NEVER_ISSUED, b"entry".to_vec())) {
        Ok(Posted::Gone) => panic!("{closed}"),
        Ok(Posted::Sent) => panic!("a post to a connection never issued was sent"),
        Err(error) => assert!(
            error.0.ends_with("BadRequestException"),
            "the post failed otherwise than NOC-278 says: {error:?}"
        ),
    }
    match runtime.block_on(api.close(NEVER_ISSUED)) {
        Ok(()) => panic!("{closed}"),
        Err(error) => assert!(
            error.0.ends_with("BadRequestException"),
            "the close failed otherwise than NOC-278 says: {error:?}"
        ),
    }
}

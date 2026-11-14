//! Against the AWS Emulator (docs/aws-emulator.md): the `connections` table
//! on DynamoDB keeps the same contract as the in-memory store, and the
//! delivery to a gone subscriber is marked as waiting for NOC-278. The tests
//! are ignored unless asked for, and refuse to run against anything but an
//! emulator on this machine.

use aws_config::BehaviorVersion;
use ncdb_relay_lambda::apigw::ApiGatewayApi;
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

#[test]
#[ignore = "needs the AWS Emulator (docs/aws-emulator.md)"]
fn the_dynamodb_store_keeps_the_connections_contract() {
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
    let store = DynamoConnections::new(db.clone(), &table, runtime.handle().clone());
    // The contract's futures run on any executor; the store's calls run on
    // the runtime it was given.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.block_on(ncdb_relay_lambda::contract::run(&store, "dynamo"));
    }));
    let _ = runtime.block_on(dynamo::delete(&db, &table));
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
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

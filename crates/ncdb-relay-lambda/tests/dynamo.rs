//! The `connections` table on DynamoDB keeps the same contract as the
//! in-memory store, against the AWS Emulator (docs/aws-emulator.md). The
//! test is ignored unless asked for, and refuses to run against anything
//! but an emulator on this machine.

use aws_config::BehaviorVersion;
use ncdb_relay_lambda::dynamo::{self, DynamoConnections};

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

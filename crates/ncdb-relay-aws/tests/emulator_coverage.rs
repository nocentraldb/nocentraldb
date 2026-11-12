//! What the AWS Emulator covers of the operations RFC 0007's AWS backend
//! needs (RFC 0007, "The AWS backend"; NOC-264 point 2, NOC-275). Each case
//! calls one operation through the AWS SDK for Rust against an emulator on
//! this machine and reports one of three outcomes: supported, behaves
//! differently (with what differs), or not implemented (HTTP 501
//! `NotImplemented`). The test prints the outcomes as a Markdown table and,
//! when `NCDB_EMULATOR_REPORT` names a file, writes the table there.
//!
//! It fails only when it cannot run: no emulator on a loopback endpoint, or
//! real AWS credentials in the environment. A gap is a result, not a
//! failure: each one becomes a ticket, so the backend does not build on
//! behaviour the emulator lacks.
//!
//! It runs only against the emulator, never against AWS: it refuses an
//! `AWS_ENDPOINT_URL` that is not a loopback address. It is `#[ignore]`d,
//! so `cargo test` never runs it by accident; docs/aws-emulator.md says how
//! to start the emulator, with telemetry off, and run it.

#[path = "emulator/apigateway.rs"]
mod apigateway;
#[path = "emulator/common.rs"]
mod common;
#[path = "emulator/dynamodb.rs"]
mod dynamodb;
#[path = "emulator/lambda.rs"]
mod lambda;
#[path = "emulator/s3.rs"]
mod s3;

use common::Outcome;

#[tokio::test]
#[ignore = "needs the AWS Emulator on this machine, with telemetry off: see docs/aws-emulator.md"]
async fn what_the_emulator_covers() {
    let url = common::endpoint();
    let config = common::config().await;
    let db = aws_sdk_dynamodb::Client::new(&config);
    let s3_client = common::s3(&config);
    let https = common::https();
    let api = aws_sdk_apigatewayv2::Client::new(&config);

    let mut rows: Vec<(&str, &str, Outcome)> = Vec::new();
    let first = dynamodb::create_table(&db).await;
    if let Outcome::Differs(what) = &first {
        assert!(
            !what.starts_with("no answer"),
            "the emulator at {url} does not answer: {what}"
        );
    }
    rows.push(("DynamoDB", "CreateTable with a GSI", first));
    rows.push((
        "DynamoDB",
        "UpdateTimeToLive",
        dynamodb::time_to_live(&db).await,
    ));
    rows.push((
        "DynamoDB",
        "TransactWriteItems, conditions hold",
        dynamodb::transaction(&db).await,
    ));
    rows.push((
        "DynamoDB",
        "TransactWriteItems, a condition fails: cancellation reasons",
        dynamodb::failed_transaction(&db).await,
    ));
    rows.push((
        "DynamoDB",
        "Query with a sort-key range",
        dynamodb::query(&db).await,
    ));
    rows.push((
        "DynamoDB",
        "Query on a GSI",
        dynamodb::query_index(&db).await,
    ));
    rows.push((
        "DynamoDB",
        "UpdateItem with a condition",
        dynamodb::conditional_update(&db).await,
    ));
    rows.push((
        "DynamoDB",
        "UpdateContinuousBackups (point-in-time recovery)",
        dynamodb::point_in_time_recovery(&db).await,
    ));
    rows.push((
        "S3",
        "PutObject and GetObject, path-style",
        s3::put_get(&s3_client).await,
    ));
    rows.push(("S3", "CopyObject", s3::copy(&s3_client).await));
    rows.push(("S3", "DeleteObject", s3::delete(&s3_client).await));
    rows.push((
        "S3",
        "Presigned PUT with a fixed content length",
        s3::presigned_put(&s3_client, &https).await,
    ));
    rows.push((
        "S3",
        "Presigned GET",
        s3::presigned_get(&s3_client, &https).await,
    ));
    rows.push((
        "S3",
        "PutBucketLifecycleConfiguration",
        s3::lifecycle(&s3_client).await,
    ));
    rows.push(("S3", "PutBucketCors", s3::cors(&s3_client).await));
    let (created, _) = apigateway::websocket_api(&api).await;
    rows.push((
        "API Gateway",
        "CreateApi (WebSocket) and CreateStage",
        created,
    ));
    let (post, delete) = apigateway::connections(&config).await;
    rows.push(("API Gateway", "@connections PostToConnection", post));
    rows.push(("API Gateway", "@connections DeleteConnection", delete));
    let (create, invoke) = lambda::lambda(&config).await;
    rows.push(("Lambda", "CreateFunction", create));
    rows.push(("Lambda", "Invoke", invoke));

    let mut table = String::from("| Service | Operation | Result |\n| --- | --- | --- |\n");
    for (service, operation, outcome) in &rows {
        let outcome = outcome.to_string().replace('|', "\\|").replace('\n', " ");
        table.push_str(&format!("| {service} | {operation} | {outcome} |\n"));
    }
    println!("{table}");
    if let Ok(path) = std::env::var("NCDB_EMULATOR_REPORT") {
        std::fs::write(path, &table).expect("the report file");
    }
}

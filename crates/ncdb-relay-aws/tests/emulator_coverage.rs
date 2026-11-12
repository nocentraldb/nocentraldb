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

use std::fmt;
use std::time::Duration;

use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::error::{ProvideErrorMetadata, SdkError};
use aws_sdk_dynamodb::types::{
    AttributeDefinition, AttributeValue, BillingMode, GlobalSecondaryIndex, KeySchemaElement,
    KeyType, PointInTimeRecoverySpecification, Projection, ProjectionType, Put,
    ScalarAttributeType, TimeToLiveSpecification, TransactWriteItem,
};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{
    BucketLifecycleConfiguration, CorsConfiguration, CorsRule, ExpirationStatus,
    LifecycleExpiration, LifecycleRule, LifecycleRuleFilter,
};

/// What the emulator did with one operation.
enum Outcome {
    Supported,
    Differs(String),
    NotImplemented,
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Outcome::Supported => f.write_str("supported"),
            Outcome::Differs(what) => write!(f, "behaves differently: {what}"),
            Outcome::NotImplemented => f.write_str("not implemented (501)"),
        }
    }
}

/// The outcome of an SDK error: not implemented when the emulator answered
/// 501 or `NotImplemented`, else a difference that names the error.
fn failed<E, R>(error: &SdkError<E, R>) -> Outcome
where
    E: ProvideErrorMetadata + fmt::Debug,
    R: fmt::Debug,
{
    let status = match error {
        SdkError::ServiceError(e) => format!("{:?}", e.raw()),
        _ => String::new(),
    };
    let code = error.code().unwrap_or_default().to_owned();
    if code == "NotImplemented" || status.contains("501") {
        return Outcome::NotImplemented;
    }
    match error {
        SdkError::ServiceError(_) => {
            Outcome::Differs(format!("{code}: {}", error.message().unwrap_or_default()))
        }
        other => Outcome::Differs(format!("no answer: {other:?}")),
    }
}

/// The emulator's endpoint, refusing anything but a loopback address, and
/// any real credentials in the environment.
fn endpoint() -> String {
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
        matches!(host, "127.0.0.1" | "localhost" | "[" | "::1"),
        "AWS_ENDPOINT_URL is not a loopback address: {url}"
    );
    url
}

async fn config() -> aws_config::SdkConfig {
    aws_config::defaults(BehaviorVersion::latest()).load().await
}

fn s3(config: &aws_config::SdkConfig) -> aws_sdk_s3::Client {
    let s3 = aws_sdk_s3::config::Builder::from(config)
        .force_path_style(true)
        .build();
    aws_sdk_s3::Client::from_conf(s3)
}

/// An HTTPS client that trusts the emulator's certificate, `SSL_CERT_FILE`,
/// for the presigned URLs.
fn https() -> reqwest::Client {
    let path = std::env::var("SSL_CERT_FILE").expect("SSL_CERT_FILE names the emulator's ca.pem");
    let pem = std::fs::read(path).expect("the emulator's ca.pem");
    let ca = reqwest::Certificate::from_pem(&pem).expect("a PEM certificate");
    reqwest::Client::builder()
        .add_root_certificate(ca)
        .build()
        .expect("an HTTPS client")
}

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
async fn create_table(db: &aws_sdk_dynamodb::Client) -> Outcome {
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
async fn time_to_live(db: &aws_sdk_dynamodb::Client) -> Outcome {
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
async fn transaction(db: &aws_sdk_dynamodb::Client) -> Outcome {
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
async fn failed_transaction(db: &aws_sdk_dynamodb::Client) -> Outcome {
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
async fn query(db: &aws_sdk_dynamodb::Client) -> Outcome {
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
async fn query_index(db: &aws_sdk_dynamodb::Client) -> Outcome {
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
async fn conditional_update(db: &aws_sdk_dynamodb::Client) -> Outcome {
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
async fn point_in_time_recovery(db: &aws_sdk_dynamodb::Client) -> Outcome {
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

const BUCKET: &str = "frames";

/// `PutObject` and `GetObject`, path-style.
async fn put_get(s3: &aws_sdk_s3::Client) -> Outcome {
    if let Err(error) = s3.create_bucket().bucket(BUCKET).send().await {
        return failed(&error);
    }
    if let Err(error) = s3
        .put_object()
        .bucket(BUCKET)
        .key("frames/s1/c1")
        .body(ByteStream::from_static(b"frame"))
        .send()
        .await
    {
        return failed(&error);
    }
    match s3
        .get_object()
        .bucket(BUCKET)
        .key("frames/s1/c1")
        .send()
        .await
    {
        Ok(out) => match out.body.collect().await {
            Ok(body) if body.clone().into_bytes().as_ref() == b"frame" => Outcome::Supported,
            Ok(_) => Outcome::Differs("another body".into()),
            Err(error) => Outcome::Differs(format!("the body: {error}")),
        },
        Err(error) => failed(&error),
    }
}

/// `CopyObject`, as an upload becomes a frame.
async fn copy(s3: &aws_sdk_s3::Client) -> Outcome {
    if let Err(error) = s3
        .copy_object()
        .bucket(BUCKET)
        .copy_source(format!("{BUCKET}/frames/s1/c1"))
        .key("frames/s1/c2")
        .send()
        .await
    {
        return failed(&error);
    }
    match s3
        .get_object()
        .bucket(BUCKET)
        .key("frames/s1/c2")
        .send()
        .await
    {
        Ok(out) => match out.body.collect().await {
            Ok(body) if body.clone().into_bytes().as_ref() == b"frame" => Outcome::Supported,
            _ => Outcome::Differs("the copy has another body".into()),
        },
        Err(error) => failed(&error),
    }
}

/// `DeleteObject`: a get afterwards is `NoSuchKey`.
async fn delete(s3: &aws_sdk_s3::Client) -> Outcome {
    if let Err(error) = s3
        .delete_object()
        .bucket(BUCKET)
        .key("frames/s1/c2")
        .send()
        .await
    {
        return failed(&error);
    }
    match s3
        .get_object()
        .bucket(BUCKET)
        .key("frames/s1/c2")
        .send()
        .await
    {
        Err(error) if error.as_service_error().is_some_and(|e| e.is_no_such_key()) => {
            Outcome::Supported
        }
        Err(error) => failed(&error),
        Ok(_) => Outcome::Differs("the object is still there".into()),
    }
}

/// A presigned PUT with the content length fixed in the signature: the
/// right length is stored, another length is refused.
async fn presigned_put(s3: &aws_sdk_s3::Client, https: &reqwest::Client) -> Outcome {
    let body = b"upload of eleven";
    let presigned = match s3
        .put_object()
        .bucket(BUCKET)
        .key("uploads/u1")
        .content_length(body.len() as i64)
        .presigned(PresigningConfig::expires_in(Duration::from_secs(900)).expect("15 minutes"))
        .await
    {
        Ok(p) => p,
        Err(error) => return Outcome::Differs(format!("presigning: {error}")),
    };
    let send = |bytes: &'static [u8]| {
        let mut request = https.put(presigned.uri());
        for (name, value) in presigned.headers() {
            if !name.eq_ignore_ascii_case("content-length") {
                request = request.header(name, value);
            }
        }
        request.body(bytes).send()
    };
    match send(body).await {
        Ok(r) if r.status().as_u16() == 501 => return Outcome::NotImplemented,
        Ok(r) if r.status().is_success() => {}
        Ok(r) => return Outcome::Differs(format!("the right length got {}", r.status())),
        Err(error) => return Outcome::Differs(format!("no answer: {error}")),
    }
    match send(b"a longer body than was signed").await {
        Ok(r) if r.status().is_success() => {
            Outcome::Differs("a body of another length was stored".into())
        }
        Ok(_) => Outcome::Supported,
        Err(error) => Outcome::Differs(format!("no answer: {error}")),
    }
}

/// A presigned GET.
async fn presigned_get(s3: &aws_sdk_s3::Client, https: &reqwest::Client) -> Outcome {
    let presigned = match s3
        .get_object()
        .bucket(BUCKET)
        .key("frames/s1/c1")
        .presigned(PresigningConfig::expires_in(Duration::from_secs(300)).expect("5 minutes"))
        .await
    {
        Ok(p) => p,
        Err(error) => return Outcome::Differs(format!("presigning: {error}")),
    };
    match https.get(presigned.uri()).send().await {
        Ok(r) if r.status().as_u16() == 501 => Outcome::NotImplemented,
        Ok(r) if r.status().is_success() => match r.bytes().await {
            Ok(b) if b.as_ref() == b"frame" => Outcome::Supported,
            _ => Outcome::Differs("another body".into()),
        },
        Ok(r) => Outcome::Differs(format!("status {}", r.status())),
        Err(error) => Outcome::Differs(format!("no answer: {error}")),
    }
}

/// A lifecycle rule that expires uploads after one day.
async fn lifecycle(s3: &aws_sdk_s3::Client) -> Outcome {
    let rule = LifecycleRule::builder()
        .id("uploads")
        .status(ExpirationStatus::Enabled)
        .filter(LifecycleRuleFilter::builder().prefix("uploads/").build())
        .expiration(LifecycleExpiration::builder().days(1).build())
        .build()
        .expect("a lifecycle rule");
    let config = BucketLifecycleConfiguration::builder()
        .rules(rule)
        .build()
        .expect("a lifecycle configuration");
    if let Err(error) = s3
        .put_bucket_lifecycle_configuration()
        .bucket(BUCKET)
        .lifecycle_configuration(config)
        .send()
        .await
    {
        return failed(&error);
    }
    match s3
        .get_bucket_lifecycle_configuration()
        .bucket(BUCKET)
        .send()
        .await
    {
        Ok(out) if out.rules().len() == 1 => Outcome::Supported,
        Ok(out) => Outcome::Differs(format!("{} rules read back", out.rules().len())),
        Err(error) => failed(&error),
    }
}

/// A CORS configuration, for browsers that put and get frames.
async fn cors(s3: &aws_sdk_s3::Client) -> Outcome {
    let rule = CorsRule::builder()
        .allowed_methods("GET")
        .allowed_methods("PUT")
        .allowed_origins("*")
        .build()
        .expect("a CORS rule");
    let config = CorsConfiguration::builder()
        .cors_rules(rule)
        .build()
        .expect("a CORS configuration");
    if let Err(error) = s3
        .put_bucket_cors()
        .bucket(BUCKET)
        .cors_configuration(config)
        .send()
        .await
    {
        return failed(&error);
    }
    match s3.get_bucket_cors().bucket(BUCKET).send().await {
        Ok(out) if out.cors_rules().len() == 1 => Outcome::Supported,
        Ok(out) => Outcome::Differs(format!("{} rules read back", out.cors_rules().len())),
        Err(error) => failed(&error),
    }
}

/// A WebSocket API with a stage; returns its id.
async fn websocket_api(api: &aws_sdk_apigatewayv2::Client) -> (Outcome, Option<String>) {
    let created = api
        .create_api()
        .name("relay")
        .protocol_type(aws_sdk_apigatewayv2::types::ProtocolType::Websocket)
        .route_selection_expression("$request.body.action")
        .send()
        .await;
    let id = match created {
        Ok(out) => match out.api_id() {
            Some(id) => id.to_owned(),
            None => return (Outcome::Differs("no api id".into()), None),
        },
        Err(error) => return (failed(&error), None),
    };
    match api
        .create_stage()
        .api_id(&id)
        .stage_name("prod")
        .send()
        .await
    {
        Ok(_) => (Outcome::Supported, Some(id)),
        Err(error) => (failed(&error), Some(id)),
    }
}

/// `@connections` `PostToConnection` and `DeleteConnection` to a connection
/// that does not exist: AWS answers `GoneException` (410).
async fn connections(config: &aws_config::SdkConfig) -> (Outcome, Outcome) {
    let client = aws_sdk_apigatewaymanagement::Client::new(config);
    let post = match client
        .post_to_connection()
        .connection_id("gone=")
        .data(aws_sdk_apigatewaymanagement::primitives::Blob::new(
            b"entry".to_vec(),
        ))
        .send()
        .await
    {
        Err(error)
            if error
                .as_service_error()
                .is_some_and(|e| e.is_gone_exception()) =>
        {
            Outcome::Supported
        }
        Err(error) => failed(&error),
        Ok(_) => Outcome::Differs("a post to an unknown connection succeeded".into()),
    };
    let delete = match client
        .delete_connection()
        .connection_id("gone=")
        .send()
        .await
    {
        Err(error)
            if error
                .as_service_error()
                .is_some_and(|e| e.is_gone_exception()) =>
        {
            Outcome::Supported
        }
        Err(error) => failed(&error),
        Ok(_) => Outcome::Differs("deleting an unknown connection succeeded".into()),
    };
    (post, delete)
}

/// A zip archive with one stored (uncompressed) file, enough for a Lambda
/// deployment package.
fn zip(name: &str, content: &[u8]) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xffff_ffffu32;
        for byte in data {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb8_8320 & (0u32.wrapping_sub(crc & 1)));
            }
        }
        !crc
    }
    let crc = crc32(content);
    let size = content.len() as u32;
    let name_len = name.len() as u16;
    let mut out = Vec::new();
    let local = |out: &mut Vec<u8>| {
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
    };
    local(&mut out);
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(content);
    let central = out.len() as u32;
    out.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
    out.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&name_len.to_le_bytes());
    out.extend_from_slice(&[0; 12]);
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(name.as_bytes());
    let central_size = out.len() as u32 - central;
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0, 1, 0, 1, 0]);
    out.extend_from_slice(&central_size.to_le_bytes());
    out.extend_from_slice(&central.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// Lambda `CreateFunction` and `Invoke`, with a Python handler that echoes
/// its event.
async fn lambda(config: &aws_config::SdkConfig) -> (Outcome, Outcome) {
    let client = aws_sdk_lambda::Client::new(config);
    let code = zip(
        "handler.py",
        b"def handle(event, context):\n    return event\n",
    );
    let created = client
        .create_function()
        .function_name("relay")
        .runtime(aws_sdk_lambda::types::Runtime::Python312)
        .role("arn:aws:iam::000000000000:role/relay")
        .handler("handler.handle")
        .code(
            aws_sdk_lambda::types::FunctionCode::builder()
                .zip_file(aws_sdk_lambda::primitives::Blob::new(code))
                .build(),
        )
        .send()
        .await;
    let create = match created {
        Ok(_) => Outcome::Supported,
        Err(error) => return (failed(&error), Outcome::Differs("no function".into())),
    };
    let invoke = match client
        .invoke()
        .function_name("relay")
        .payload(aws_sdk_lambda::primitives::Blob::new(
            br#"{"action":"ping"}"#.to_vec(),
        ))
        .send()
        .await
    {
        Ok(out) => match out.payload().map(|p| p.as_ref().to_vec()) {
            Some(p) if p == br#"{"action": "ping"}"# || p == br#"{"action":"ping"}"# => {
                Outcome::Supported
            }
            Some(p) => Outcome::Differs(format!(
                "payload {:?}{}",
                String::from_utf8_lossy(&p),
                out.function_error()
                    .map(|e| format!(", function error {e}"))
                    .unwrap_or_default()
            )),
            None => Outcome::Differs("no payload".into()),
        },
        Err(error) => failed(&error),
    };
    (create, invoke)
}

#[tokio::test]
#[ignore = "needs the AWS Emulator on this machine, with telemetry off: see docs/aws-emulator.md"]
async fn what_the_emulator_covers() {
    let url = endpoint();
    let config = config().await;
    let db = aws_sdk_dynamodb::Client::new(&config);
    let s3 = s3(&config);
    let https = https();
    let api = aws_sdk_apigatewayv2::Client::new(&config);

    let mut rows: Vec<(&str, &str, Outcome)> = Vec::new();
    let first = create_table(&db).await;
    if let Outcome::Differs(what) = &first {
        assert!(
            !what.starts_with("no answer"),
            "the emulator at {url} does not answer: {what}"
        );
    }
    rows.push(("DynamoDB", "CreateTable with a GSI", first));
    rows.push(("DynamoDB", "UpdateTimeToLive", time_to_live(&db).await));
    rows.push((
        "DynamoDB",
        "TransactWriteItems, conditions hold",
        transaction(&db).await,
    ));
    rows.push((
        "DynamoDB",
        "TransactWriteItems, a condition fails: cancellation reasons",
        failed_transaction(&db).await,
    ));
    rows.push(("DynamoDB", "Query with a sort-key range", query(&db).await));
    rows.push(("DynamoDB", "Query on a GSI", query_index(&db).await));
    rows.push((
        "DynamoDB",
        "UpdateItem with a condition",
        conditional_update(&db).await,
    ));
    rows.push((
        "DynamoDB",
        "UpdateContinuousBackups (point-in-time recovery)",
        point_in_time_recovery(&db).await,
    ));
    rows.push((
        "S3",
        "PutObject and GetObject, path-style",
        put_get(&s3).await,
    ));
    rows.push(("S3", "CopyObject", copy(&s3).await));
    rows.push(("S3", "DeleteObject", delete(&s3).await));
    rows.push((
        "S3",
        "Presigned PUT with a fixed content length",
        presigned_put(&s3, &https).await,
    ));
    rows.push(("S3", "Presigned GET", presigned_get(&s3, &https).await));
    rows.push((
        "S3",
        "PutBucketLifecycleConfiguration",
        lifecycle(&s3).await,
    ));
    rows.push(("S3", "PutBucketCors", cors(&s3).await));
    let (created, _) = websocket_api(&api).await;
    rows.push((
        "API Gateway",
        "CreateApi (WebSocket) and CreateStage",
        created,
    ));
    let (post, delete) = connections(&config).await;
    rows.push(("API Gateway", "@connections PostToConnection", post));
    rows.push(("API Gateway", "@connections DeleteConnection", delete));
    let (create, invoke) = lambda(&config).await;
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

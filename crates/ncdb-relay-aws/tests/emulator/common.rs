//! What every case of the coverage check shares: its outcome, how an SDK
//! error becomes one, and the clients, which reach only an emulator on this
//! machine.

use std::fmt;

use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::error::{ProvideErrorMetadata, SdkError};

/// What the emulator did with one operation.
pub enum Outcome {
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
pub fn failed<E, R>(error: &SdkError<E, R>) -> Outcome
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
pub fn endpoint() -> String {
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

pub async fn config() -> aws_config::SdkConfig {
    aws_config::defaults(BehaviorVersion::latest()).load().await
}

pub fn s3(config: &aws_config::SdkConfig) -> aws_sdk_s3::Client {
    let s3 = aws_sdk_s3::config::Builder::from(config)
        .force_path_style(true)
        .build();
    aws_sdk_s3::Client::from_conf(s3)
}

/// An HTTPS client that trusts the emulator's certificate, `SSL_CERT_FILE`,
/// for the presigned URLs.
pub fn https() -> reqwest::Client {
    let path = std::env::var("SSL_CERT_FILE").expect("SSL_CERT_FILE names the emulator's ca.pem");
    let pem = std::fs::read(path).expect("the emulator's ca.pem");
    let ca = reqwest::Certificate::from_pem(&pem).expect("a PEM certificate");
    reqwest::Client::builder()
        .add_root_certificate(ca)
        .build()
        .expect("an HTTPS client")
}

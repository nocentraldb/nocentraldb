//! The connection API client against a local endpoint that answers as API
//! Gateway's `@connections` does: 200 for a post it took, 410
//! `GoneException` for a connection that is gone, 403 `ForbiddenException`
//! for a caller it refuses. The AWS SDK reads each answer itself, so the
//! test covers how the client maps them. The AWS Emulator cannot: it
//! serves no WebSocket data plane (NOC-278).

use std::sync::{Arc, Mutex};

use aws_sdk_apigatewaymanagement::config::{BehaviorVersion, Credentials, Region};
use ncdb_relay_lambda::apigw::ApiGatewayApi;
use ncdb_relay_lambda::{ConnectionApi, Posted};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// The request lines the endpoint saw, and its address.
async fn endpoint() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let log = log.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 64 * 1024];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).into_owned();
                let line = request.lines().next().unwrap_or_default().to_owned();
                log.lock().unwrap().push(line.clone());
                let (status, kind) = if line.contains("gone") {
                    ("410 Gone", Some("GoneException"))
                } else if line.contains("refused") {
                    ("403 Forbidden", Some("ForbiddenException"))
                } else {
                    ("200 OK", None)
                };
                let body = kind.map_or(String::new(), |_| "{\"message\":\"no\"}".into());
                let header = kind.map_or(String::new(), |k| format!("x-amzn-ErrorType: {k}\r\n"));
                let answer = format!(
                    "HTTP/1.1 {status}\r\n{header}Content-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(answer.as_bytes()).await;
            });
        }
    });
    (format!("http://{addr}"), seen)
}

async fn api(url: &str) -> ApiGatewayApi {
    let config = aws_sdk_apigatewaymanagement::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new("eu-central-1"))
        .credentials_provider(Credentials::new("test", "test", None, None, "test"))
        .endpoint_url(url)
        .build();
    ApiGatewayApi::new(
        aws_sdk_apigatewaymanagement::Client::from_conf(config),
        tokio::runtime::Handle::current(),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn a_post_is_sent_to_its_connection() {
    let (url, seen) = endpoint().await;
    let api = api(&url).await;
    assert_eq!(
        api.post("open", b"entry".to_vec()).await.unwrap(),
        Posted::Sent
    );
    assert_eq!(seen.lock().unwrap()[0], "POST /@connections/open HTTP/1.1");
}

/// Stands in for the emulator until NOC-278 closes: see
/// `delivery_to_a_gone_subscriber_waits_for_noc_278` in `tests/emulator.rs`.
#[tokio::test(flavor = "multi_thread")]
async fn a_gone_connection_is_reported_gone_and_its_close_is_done() {
    let (url, seen) = endpoint().await;
    let api = api(&url).await;
    assert_eq!(
        api.post("gone", b"entry".to_vec()).await.unwrap(),
        Posted::Gone
    );
    api.close("gone")
        .await
        .expect("closing a gone connection is done");
    assert_eq!(
        seen.lock().unwrap()[1],
        "DELETE /@connections/gone HTTP/1.1"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refusal_is_an_error_that_names_only_its_code() {
    let (url, _) = endpoint().await;
    let api = api(&url).await;
    let error = api
        .post("refused", b"secret frame".to_vec())
        .await
        .unwrap_err();
    assert!(error.0.contains("ForbiddenException"), "{}", error.0);
    assert!(!error.0.contains("secret"), "{}", error.0);
    assert!(api.close("refused").await.is_err());
}

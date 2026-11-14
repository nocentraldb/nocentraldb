//! API Gateway: a WebSocket API with a stage, and the `@connections` API
//! the relay sends live entries through.

use super::common::{Outcome, failed};

/// A connection id in API Gateway's form, 16 characters of base64 ending in
/// `=`, of a connection that never existed.
const GONE: &str = "L0SM9cOFvHcCIhw=";

/// A WebSocket API with a stage; returns its id and its endpoint.
pub async fn websocket_api(
    api: &aws_sdk_apigatewayv2::Client,
) -> (Outcome, Option<(String, String)>) {
    let created = api
        .create_api()
        .name("relay")
        .protocol_type(aws_sdk_apigatewayv2::types::ProtocolType::Websocket)
        .route_selection_expression("$request.body.action")
        .send()
        .await;
    let (id, endpoint) = match created {
        Ok(out) => match (out.api_id(), out.api_endpoint()) {
            (Some(id), Some(endpoint)) => (id.to_owned(), endpoint.to_owned()),
            _ => return (Outcome::Differs("no api id or endpoint".into()), None),
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
        Ok(_) => (Outcome::Supported, Some((id, endpoint))),
        Err(error) => (failed(&error), Some((id, endpoint))),
    }
}

/// `@connections` `PostToConnection` and `DeleteConnection` to a connection
/// that does not exist: AWS answers `GoneException` (410).
pub async fn connections(config: &aws_config::SdkConfig) -> (Outcome, Outcome) {
    let client = aws_sdk_apigatewaymanagement::Client::new(config);
    let post = match client
        .post_to_connection()
        .connection_id(GONE)
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
    let delete = match client.delete_connection().connection_id(GONE).send().await {
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

/// NOC-278: a WebSocket upgrade on the stage, as a member's client opens
/// its connection. AWS answers `101 Switching Protocols`. Until the emulator
/// does, it issues no connection id, so a post to a live connection and a
/// post after its close (410) cannot be probed; the day it answers 101, this
/// row says so, and that probe can be written.
pub async fn upgrade(https: &reqwest::Client, endpoint: &str) -> Outcome {
    let url = format!(
        "{}/prod",
        endpoint
            .replacen("wss://", "https://", 1)
            .replacen("ws://", "http://", 1)
            .trim_end_matches('/')
    );
    let answer = https
        .get(&url)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .send()
        .await;
    match answer {
        Ok(answer) if answer.status().as_u16() == 101 => Outcome::Supported,
        Ok(answer) if answer.status().as_u16() == 501 => Outcome::NotImplemented,
        Ok(answer) => Outcome::Differs(format!(
            "{url} answers {} to the upgrade, not 101 (NOC-278)",
            answer.status()
        )),
        Err(error) => Outcome::Differs(format!("no answer at {url}: {error} (NOC-278)")),
    }
}

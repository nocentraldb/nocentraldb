//! API Gateway: a WebSocket API with a stage, and the `@connections` API
//! the relay sends live entries through.

use super::common::{Outcome, failed};

/// A connection id in API Gateway's form, 16 characters of base64 ending in
/// `=`, of a connection that never existed.
const GONE: &str = "L0SM9cOFvHcCIhw=";

/// A WebSocket API with a stage; returns its id.
pub async fn websocket_api(api: &aws_sdk_apigatewayv2::Client) -> (Outcome, Option<String>) {
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

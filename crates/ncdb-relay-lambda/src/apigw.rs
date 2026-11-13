//! API Gateway's connection API, `@connections` (RFC 0007, "Delivery to
//! subscribers"): `PostToConnection` sends one binary message to a
//! connection, `DeleteConnection` closes one. A connection that is gone
//! answers `GoneException` (410): a post reports [`Posted::Gone`], and the
//! handler forgets the connection; a close of a gone connection is done.
//!
//! The client's endpoint is the API's stage URL,
//! `https://<api id>.execute-api.<region>.amazonaws.com/<stage>`.

use aws_sdk_apigatewaymanagement::Client;
use aws_sdk_apigatewaymanagement::primitives::Blob;
use ncdb_relay::store::StoreError;
use tokio::runtime::Handle;

use crate::connections::{ConnectionApi, Posted};

/// The connection API of one WebSocket API stage, through a client whose
/// calls run on `runtime`.
#[derive(Clone)]
pub struct ApiGatewayApi {
    client: Client,
    runtime: Handle,
}

impl std::fmt::Debug for ApiGatewayApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiGatewayApi").finish_non_exhaustive()
    }
}

impl ApiGatewayApi {
    /// The connection API `client` reaches, whose calls run on `runtime`.
    pub fn new(client: Client, runtime: Handle) -> ApiGatewayApi {
        ApiGatewayApi { client, runtime }
    }
}

fn stopped<T>(_: T) -> StoreError {
    StoreError("a call of the connection API stopped".into())
}

impl ConnectionApi for ApiGatewayApi {
    fn post(
        &self,
        id: &str,
        message: Vec<u8>,
    ) -> impl Future<Output = Result<Posted, StoreError>> + Send {
        let (client, id) = (self.client.clone(), id.to_owned());
        let call = self.runtime.spawn(async move {
            let sent = client
                .post_to_connection()
                .connection_id(id)
                .data(Blob::new(message))
                .send()
                .await;
            match sent {
                Ok(_) => Ok(Posted::Sent),
                Err(error)
                    if error
                        .as_service_error()
                        .is_some_and(|e| e.is_gone_exception()) =>
                {
                    Ok(Posted::Gone)
                }
                // Only the error's code reaches the message: never the
                // message posted.
                Err(error) => Err(StoreError(format!(
                    "the connection API refused a post: {}",
                    aws_sdk_apigatewaymanagement::error::ProvideErrorMetadata::code(&error)
                        .unwrap_or("no answer")
                ))),
            }
        });
        async move { call.await.unwrap_or_else(|e| Err(stopped(e))) }
    }

    fn close(&self, id: &str) -> impl Future<Output = Result<(), StoreError>> + Send {
        let (client, id) = (self.client.clone(), id.to_owned());
        let call = self.runtime.spawn(async move {
            match client.delete_connection().connection_id(id).send().await {
                Ok(_) => Ok(()),
                Err(error)
                    if error
                        .as_service_error()
                        .is_some_and(|e| e.is_gone_exception()) =>
                {
                    Ok(())
                }
                Err(error) => Err(StoreError(format!(
                    "the connection API refused a close: {}",
                    aws_sdk_apigatewaymanagement::error::ProvideErrorMetadata::code(&error)
                        .unwrap_or("no answer")
                ))),
            }
        });
        async move { call.await.unwrap_or_else(|e| Err(stopped(e))) }
    }
}

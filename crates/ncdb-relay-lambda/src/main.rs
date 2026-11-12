//! The Lambda function: Lambda's loop of invocations around the handler.
//! Each invocation is one API Gateway WebSocket event; the function
//! answers every one with status 200, and its answers to the member go
//! out through the connection API, never in the response, since API
//! Gateway's route responses carry text, not RFC 0007's binary messages.
//! It prints each invocation's log line to stdout, which Lambda sends to
//! CloudWatch Logs.
//!
//! **It does not start yet.** Besides the DynamoDB and S3 backend
//! (`ncdb-relay-aws`, NOC-270), the function needs a `connections` store on
//! DynamoDB and a client of the real connection API (NOC-272); until they
//! exist, it says so and exits, and the handler runs only in the local
//! harness.

use std::process::ExitCode;
use std::sync::Arc;

use aws_lambda_events::apigw::{ApiGatewayProxyResponse, ApiGatewayWebsocketProxyRequest};
use lambda_runtime::{Error, LambdaEvent, service_fn};
use ncdb_relay::store::RelayStore;
use ncdb_relay_lambda::{ConnectionApi, Connections, Event, Handler, Urls};

/// Runs Lambda's loop of invocations around `handler`.
#[expect(
    dead_code,
    reason = "the function starts once the connections table and the connection API client exist (NOC-272)"
)]
async fn serve<S, C, A, U>(handler: Handler<S, C, A, U>) -> Result<(), Error>
where
    S: RelayStore + 'static,
    C: Connections + 'static,
    A: ConnectionApi + 'static,
    U: Urls + 'static,
{
    let handler = Arc::new(handler);
    lambda_runtime::run(service_fn(
        move |invocation: LambdaEvent<ApiGatewayWebsocketProxyRequest>| {
            let handler = handler.clone();
            async move {
                match Event::from_request(&invocation.payload) {
                    Ok(event) => println!("{}", handler.handle(event).await),
                    Err(not) => println!("{{\"refused\":\"{not:?}\"}}"),
                }
                let mut response = ApiGatewayProxyResponse::default();
                response.status_code = 200;
                Ok::<_, Error>(response)
            }
        },
    ))
    .await
}

fn main() -> ExitCode {
    eprintln!(
        "ncdb-relay-lambda: not deployable yet: the connections table on DynamoDB \
         and the connection API client (NOC-272) are not built"
    );
    ExitCode::FAILURE
}

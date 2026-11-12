//! The Lambda function: Lambda's loop of invocations around the handler,
//! on AWS: the relay core over the DynamoDB and S3 backend
//! (`ncdb-relay-aws`), the `connections` table, the connection API, and
//! presigned S3 URLs for large changes. Its settings come from the
//! environment (`ncdb_relay_lambda::settings`); its credentials are the
//! function's role, which Lambda hands it.
//!
//! Each invocation is one API Gateway WebSocket event. The function answers
//! every one with status 200, and its answers to the member go out through
//! the connection API, never in the response, since API Gateway's route
//! responses carry text, not RFC 0007's binary messages. It prints each
//! invocation's log line to stdout, which Lambda sends to CloudWatch Logs.

// The future of `main` nests the AWS clients' futures deeper than the
// compiler's default of 128 lets it lay out.
#![recursion_limit = "256"]

use std::process::ExitCode;
use std::sync::Arc;

use aws_config::BehaviorVersion;
use aws_lambda_events::apigw::{ApiGatewayProxyResponse, ApiGatewayWebsocketProxyRequest};
use lambda_runtime::{Error, LambdaEvent, service_fn};
use ncdb_relay::store::RelayStore;
use ncdb_relay::{Config, Relay};
use ncdb_relay_aws::AwsStore;
use ncdb_relay_lambda::apigw::ApiGatewayApi;
use ncdb_relay_lambda::dynamo::DynamoConnections;
use ncdb_relay_lambda::s3::S3Urls;
use ncdb_relay_lambda::settings::Settings;
use ncdb_relay_lambda::{ConnectionApi, Connections, Event, Handler, Urls};
use tokio::runtime::Handle;

/// Runs Lambda's loop of invocations around `handler`.
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

#[tokio::main]
async fn main() -> ExitCode {
    let settings = match Settings::from_vars(|name| std::env::var(name).ok()) {
        Ok(settings) => settings,
        Err(refused) => {
            eprintln!("ncdb-relay-lambda: {refused}");
            return ExitCode::FAILURE;
        }
    };
    // Lambda sets this for the function; elsewhere the runtime would panic.
    if std::env::var_os("AWS_LAMBDA_RUNTIME_API").is_none() {
        eprintln!(
            "ncdb-relay-lambda: runs only inside AWS Lambda (AWS_LAMBDA_RUNTIME_API is unset)"
        );
        return ExitCode::FAILURE;
    }
    let aws = aws_config::defaults(BehaviorVersion::latest()).load().await;
    let runtime = Handle::current();
    let db = aws_sdk_dynamodb::Client::new(&aws);
    let s3 = aws_sdk_s3::Client::new(&aws);
    let store = AwsStore::new(
        db.clone(),
        s3.clone(),
        settings.names.clone(),
        runtime.clone(),
    );
    let mut config = Config::new(settings.allowed_accounts);
    config.max_frame = ncdb_relay::MAX_FRAME;
    let connections = DynamoConnections::new(db, settings.connections, runtime.clone());
    let api = aws_sdk_apigatewaymanagement::config::Builder::from(&aws)
        .endpoint_url(settings.connections_url)
        .build();
    let api = ApiGatewayApi::new(
        aws_sdk_apigatewaymanagement::Client::from_conf(api),
        runtime,
    );
    let urls = S3Urls::new(s3, settings.names.bucket);
    let handler = Handler::new(
        Relay::new(store, config),
        settings.relay_id,
        connections,
        api,
        urls,
    );
    match serve(handler).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ncdb-relay-lambda: the runtime ended: {error}");
            ExitCode::FAILURE
        }
    }
}

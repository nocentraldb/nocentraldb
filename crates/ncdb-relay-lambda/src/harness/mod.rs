//! A local stand-in for API Gateway's WebSocket API, so the handler runs
//! end to end without AWS: a WebSocket listener that turns each opened
//! connection, each message and each close into the event API Gateway
//! would send, built from the recorded events in `events/` and read back
//! through `aws_lambda_events`, as Lambda reads them; it calls the
//! handler with each and serves its posts through a fake connection API
//! that writes to the sockets.
//!
//! Like API Gateway, it refuses a message above 128 KiB by closing the
//! connection, closes every connection after its maximum age (2 hours,
//! shorter in tests) and then sends `$disconnect`, and hands each
//! connection's messages to the handler one at a time. A second listener
//! serves the upload and download URLs of large changes, which S3's
//! presigned URLs serve on AWS (`http`).

mod http;

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aws_lambda_events::apigw::ApiGatewayWebsocketProxyRequest;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use futures_util::{SinkExt, StreamExt};
use ncdb_core::ids::SpaceId;
use ncdb_relay::page::{Downloads, TokenDownloads};
use ncdb_relay::store::{Entry, StoreError, Token, TokenUrls, UploadId};
use ncdb_relay::wire::MAX_MESSAGE;
use ncdb_relay::{ErrorCode, Relay};
use serde_json::Value;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, WebSocketConfig};

use crate::connections::{ConnectionApi, MAX_CONNECTION_AGE, MemoryConnections, Posted};
use crate::event::Event;
use crate::handler::{Handler, Urls};

const CONNECT: &str = include_str!("events/connect.json");
const MESSAGE: &str = include_str!("events/message.json");
const DISCONNECT: &str = include_str!("events/disconnect.json");

/// The handler as the harness runs it.
type Local<S> = Handler<S, MemoryConnections, FakeApi, LocalUrls>;

/// What the fake connection API hands a connection's writer.
enum Out {
    Message(Vec<u8>),
    /// Close the connection with this status code.
    Close(CloseCode),
}

/// API Gateway's connection API, in memory: a post goes to the socket of
/// its connection, or is `Gone` when the connection is closed.
#[derive(Clone, Default)]
pub struct FakeApi {
    open: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Out>>>>,
}

impl FakeApi {
    fn sender(&self, id: &str) -> Option<mpsc::UnboundedSender<Out>> {
        let open = self.open.lock().unwrap_or_else(|p| p.into_inner());
        open.get(id).cloned()
    }

    fn list(&self, id: &str, out: mpsc::UnboundedSender<Out>) {
        let mut open = self.open.lock().unwrap_or_else(|p| p.into_inner());
        open.insert(id.to_owned(), out);
    }

    fn unlist(&self, id: &str) {
        let mut open = self.open.lock().unwrap_or_else(|p| p.into_inner());
        open.remove(id);
    }
}

impl ConnectionApi for FakeApi {
    async fn post(&self, id: &str, message: Vec<u8>) -> Result<Posted, StoreError> {
        if message.len() > MAX_MESSAGE {
            return Err(StoreError("a post above 128 KiB".into()));
        }
        match self.sender(id) {
            Some(out) if out.send(Out::Message(message)).is_ok() => Ok(Posted::Sent),
            _ => Ok(Posted::Gone),
        }
    }

    async fn close(&self, id: &str) -> Result<(), StoreError> {
        if let Some(out) = self.sender(id) {
            let _ = out.send(Out::Close(CloseCode::Normal));
        }
        Ok(())
    }
}

/// The URLs of large changes on the harness's second listener.
pub struct LocalUrls {
    base: String,
}

impl<S: TokenUrls> Urls<S> for LocalUrls {
    async fn upload(&self, _: UploadId, token: Token, _: u64) -> Result<String, ErrorCode> {
        Ok(format!(
            "{}{}{}",
            self.base,
            http::UPLOADS,
            http::hex(&token)
        ))
    }

    async fn download(
        &self,
        relay: &Relay<S>,
        space: SpaceId,
        entry: &Entry,
    ) -> Result<String, ErrorCode> {
        let prefix = format!("{}{}", self.base, http::FRAMES);
        TokenDownloads {
            relay,
            prefix: &prefix,
        }
        .url(space, entry)
        .await
    }
}

/// Ends the maximum age of every connection open now, at once (NOC-309):
/// a test that needs a connection to reach its age at a point of its own,
/// after its answers, says so instead of racing a timer.
#[derive(Clone)]
pub struct Expiry(Arc<watch::Sender<()>>);

impl Expiry {
    /// Every connection open now closes as one at its maximum age does.
    pub fn expire(&self) {
        self.0.send_replace(());
    }
}

/// The gateway: API Gateway's WebSocket API and S3's URLs, locally.
pub struct Gateway<S> {
    listener: TcpListener,
    urls: TcpListener,
    handler: Arc<Local<S>>,
    api: FakeApi,
    max_age: Duration,
    expiry: Expiry,
    log: Arc<Mutex<Vec<String>>>,
}

impl<S: TokenUrls + 'static> Gateway<S> {
    /// A gateway on `addr`, and its URLs on another port of the same
    /// host, in front of the handler over `relay`, whose deployment has
    /// the stable id `relay_id`.
    pub async fn bind(
        addr: impl ToSocketAddrs,
        relay: Relay<S>,
        relay_id: [u8; 16],
    ) -> io::Result<Gateway<S>> {
        let listener = TcpListener::bind(addr).await?;
        let mut urls_addr = listener.local_addr()?;
        urls_addr.set_port(0);
        let urls = TcpListener::bind(urls_addr).await?;
        let base = format!("http://{}", urls.local_addr()?);
        let api = FakeApi::default();
        let handler = Handler::new(
            relay,
            relay_id,
            MemoryConnections::default(),
            api.clone(),
            LocalUrls { base },
        );
        Ok(Gateway {
            listener,
            urls,
            handler: Arc::new(handler),
            api,
            max_age: Duration::from_millis(MAX_CONNECTION_AGE),
            expiry: Expiry(Arc::new(watch::channel(()).0)),
            log: Arc::default(),
        })
    }

    /// The gateway closes each connection after `max_age`, instead of API
    /// Gateway's 2 hours.
    pub fn with_max_age(mut self, max_age: Duration) -> Gateway<S> {
        self.max_age = max_age;
        self
    }

    /// What ends the connections' maximum age before it runs out.
    pub fn expiry(&self) -> Expiry {
        self.expiry.clone()
    }

    /// The address of the WebSocket API.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// The handler.
    pub fn handler(&self) -> Arc<Local<S>> {
        self.handler.clone()
    }

    /// The log lines written so far, shared with the running gateway.
    pub fn log(&self) -> Arc<Mutex<Vec<String>>> {
        self.log.clone()
    }

    /// Serves connections and URLs until the listeners fail.
    pub async fn run(self) -> io::Result<()> {
        let relay_handler = self.handler.clone();
        tokio::spawn(http::serve(self.urls, relay_handler));
        loop {
            let (stream, _) = self.listener.accept().await?;
            let link = Link {
                handler: self.handler.clone(),
                api: self.api.clone(),
                max_age: self.max_age,
                expired: self.expiry.0.subscribe(),
                log: self.log.clone(),
            };
            tokio::spawn(link.serve(stream));
        }
    }
}

/// One connection through the gateway.
struct Link<S> {
    handler: Arc<Local<S>>,
    api: FakeApi,
    max_age: Duration,
    /// Changes when the test ends the connections' age.
    expired: watch::Receiver<()>,
    log: Arc<Mutex<Vec<String>>>,
}

impl<S: TokenUrls + 'static> Link<S> {
    async fn serve(mut self, stream: TcpStream) {
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_MESSAGE))
            .max_frame_size(Some(MAX_MESSAGE));
        let Ok(ws) = tokio_tungstenite::accept_async_with_config(stream, Some(config)).await else {
            return;
        };
        let (mut sink, mut incoming) = ws.split();
        let id = connection_id();
        let connected_at = now();
        let (out, mut outgoing) = mpsc::unbounded_channel();
        self.api.list(&id, out);
        self.invoke(event(CONNECT, &id, connected_at, None)).await;
        let mut writer = tokio::spawn(async move {
            while let Some(next) = outgoing.recv().await {
                match next {
                    Out::Message(bytes) => {
                        if sink.send(Message::Binary(bytes.into())).await.is_err() {
                            return;
                        }
                    }
                    Out::Close(code) => {
                        let frame = CloseFrame {
                            code,
                            reason: "".into(),
                        };
                        let _ = sink.send(Message::Close(Some(frame))).await;
                        let _ = sink.close().await;
                        return;
                    }
                }
            }
        });
        let expired = tokio::time::sleep(self.max_age);
        tokio::pin!(expired);
        let mut too_large = false;
        loop {
            let message = tokio::select! {
                message = incoming.next() => message,
                _ = &mut writer => break,
                () = &mut expired => break,
                // Only a change ends the age: a gateway that stopped
                // running drops the sender, which is no expiry.
                Ok(()) = self.expired.changed() => break,
            };
            let body = match message {
                Some(Ok(Message::Binary(bytes))) => Some((bytes.to_vec(), true)),
                Some(Ok(Message::Text(text))) => Some((text.as_bytes().to_vec(), false)),
                Some(Err(tokio_tungstenite::tungstenite::Error::Capacity(_))) => {
                    too_large = true;
                    break;
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => continue,
            };
            self.invoke(event(MESSAGE, &id, connected_at, body)).await;
        }
        // API Gateway closes with 1009 a connection that sent a message
        // above its limit, and with 1001 one that reached its maximum age.
        let code = if too_large {
            CloseCode::Size
        } else {
            CloseCode::Away
        };
        if let Some(out) = self.api.sender(&id) {
            let _ = out.send(Out::Close(code));
        }
        self.api.unlist(&id);
        let _ = tokio::time::timeout(Duration::from_secs(1), writer).await;
        self.invoke(event(DISCONNECT, &id, connected_at, None))
            .await;
    }

    /// Calls the handler on a thread that may block, as the SQLite backend
    /// does, and keeps its log line.
    async fn invoke(&self, request: ApiGatewayWebsocketProxyRequest) {
        let Ok(event) = Event::from_request(&request) else {
            return;
        };
        let handler = self.handler.clone();
        let runtime = tokio::runtime::Handle::current();
        // A runtime shutting down, at the end of a test, cancels the call.
        let Ok(line) =
            tokio::task::spawn_blocking(move || runtime.block_on(handler.handle(event))).await
        else {
            return;
        };
        let mut log = self.log.lock().unwrap_or_else(|p| p.into_inner());
        log.push(line.to_string());
    }
}

/// The event API Gateway sends, from a recorded one: its connection id,
/// times and body replaced, read back as Lambda reads it.
fn event(
    recorded: &str,
    id: &str,
    connected_at: u64,
    body: Option<(Vec<u8>, bool)>,
) -> ApiGatewayWebsocketProxyRequest {
    let mut event: Value = serde_json::from_str(recorded).expect("a recorded event");
    let context = &mut event["requestContext"];
    context["connectionId"] = Value::from(id);
    context["connectedAt"] = Value::from(connected_at);
    context["requestTimeEpoch"] = Value::from(now());
    if let Some((bytes, binary)) = body {
        event["isBase64Encoded"] = Value::from(binary);
        event["body"] = if binary {
            Value::from(STANDARD.encode(bytes))
        } else {
            Value::from(String::from_utf8_lossy(&bytes).into_owned())
        };
    }
    serde_json::from_value(event).expect("an event Lambda reads")
}

/// A new connection id, shaped like API Gateway's: 16 characters of
/// base64.
fn connection_id() -> String {
    let mut bytes = [0u8; 10];
    getrandom::fill(&mut bytes).expect("a secure generator");
    let mut id = STANDARD.encode(bytes);
    id.truncate(15);
    id.push('=');
    id
}

/// Milliseconds since the Unix epoch.
fn now() -> u64 {
    let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    since.map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

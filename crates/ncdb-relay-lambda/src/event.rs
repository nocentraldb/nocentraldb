//! An API Gateway WebSocket event, as the handler reads it: which route,
//! which connection, the message's bytes and when API Gateway received it.
//! API Gateway hands a binary WebSocket message to Lambda base64-encoded,
//! with `isBase64Encoded` set; a text message arrives as its text.

use aws_lambda_events::apigw::ApiGatewayWebsocketProxyRequest;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// The three routes of RFC 0007's WebSocket API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// `$connect`: a member opened a connection.
    Connect,
    /// `$disconnect`: the connection closed, by either side or by API
    /// Gateway's 2-hour limit.
    Disconnect,
    /// `$default`: a message on an open connection.
    Message,
}

impl Route {
    /// The route as the log names it.
    pub fn name(self) -> &'static str {
        match self {
            Route::Connect => "connect",
            Route::Disconnect => "disconnect",
            Route::Message => "message",
        }
    }
}

/// What a message carried.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    /// No message: `$connect` and `$disconnect`.
    None,
    /// A binary message, decoded from base64.
    Binary(Vec<u8>),
    /// A text message, which RFC 0007 never sends; only its size is kept.
    Text(usize),
}

impl Body {
    /// The message's size in bytes.
    pub fn len(&self) -> usize {
        match self {
            Body::None => 0,
            Body::Binary(bytes) => bytes.len(),
            Body::Text(len) => *len,
        }
    }

    /// Whether the message is empty or absent.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// One event of the WebSocket API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    /// The route.
    pub route: Route,
    /// API Gateway's id of the connection.
    pub connection: String,
    /// The message.
    pub body: Body,
    /// When API Gateway received the event, in milliseconds since the Unix
    /// epoch.
    pub at: u64,
    /// When the connection was opened, in milliseconds since the Unix epoch.
    pub connected_at: u64,
}

/// Why a Lambda event is not an event of the WebSocket API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotAnEvent {
    /// The event names no connection.
    NoConnection,
    /// The event names no event type, or one the API does not send.
    UnknownType,
    /// A binary message whose base64 does not decode.
    BadBase64,
}

impl Event {
    /// The event API Gateway sent for one route.
    pub fn from_request(request: &ApiGatewayWebsocketProxyRequest) -> Result<Event, NotAnEvent> {
        let context = &request.request_context;
        let connection = context
            .connection_id
            .clone()
            .filter(|id| !id.is_empty())
            .ok_or(NotAnEvent::NoConnection)?;
        let route = match context.event_type.as_deref() {
            Some("CONNECT") => Route::Connect,
            Some("DISCONNECT") => Route::Disconnect,
            Some("MESSAGE") => Route::Message,
            _ => return Err(NotAnEvent::UnknownType),
        };
        let body = match (&request.body, route) {
            (_, Route::Connect | Route::Disconnect) | (None, Route::Message) => Body::None,
            (Some(body), Route::Message) if request.is_base64_encoded => {
                Body::Binary(STANDARD.decode(body).map_err(|_| NotAnEvent::BadBase64)?)
            }
            (Some(body), Route::Message) => Body::Text(body.len()),
        };
        Ok(Event {
            route,
            connection,
            body,
            at: u64::try_from(context.request_time_epoch).unwrap_or(0),
            connected_at: u64::try_from(context.connected_at).unwrap_or(0),
        })
    }
}

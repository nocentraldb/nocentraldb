//! The connection to the relay (RFC 0007, "The protocol"; NOC-175): binary
//! messages each way. A session talks to the relay only through
//! [`Transport`], so the same session runs over tokio-tungstenite natively
//! and, later, over the browser's WebSocket on wasm32.

/// A connection to the relay that carries binary messages.
#[allow(async_fn_in_trait)]
pub(crate) trait Transport {
    /// Sends one binary message, or fails when the connection is gone.
    async fn send(&mut self, message: Vec<u8>) -> Result<(), Closed>;

    /// The next binary message; `None` once the connection ended, or the
    /// relay sent anything other than a binary message or a control frame.
    async fn recv(&mut self) -> Option<Vec<u8>>;
}

/// The connection is gone.
#[derive(Debug)]
pub(crate) struct Closed;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::connect;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpStream;
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
    use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

    use super::{Closed, Transport};
    use crate::sync::wire;

    /// A WebSocket to the relay through tokio-tungstenite.
    pub(crate) struct Native(WebSocketStream<MaybeTlsStream<TcpStream>>);

    /// Opens a WebSocket to the relay at `url`, or `None` when it cannot
    /// be opened. RFC 0007's largest message is the limit on every
    /// backend, so a larger one ends the connection instead of being read;
    /// `wss://` verifies the relay against the platform's trust store.
    pub(crate) async fn connect(url: &str) -> Option<Native> {
        let config = WebSocketConfig::default()
            .max_message_size(Some(wire::MAX_MESSAGE))
            .max_frame_size(Some(wire::MAX_MESSAGE));
        let tls = crate::sync::tls::config()?;
        let connector = tokio_tungstenite::Connector::Rustls(tls);
        let (ws, _) = tokio_tungstenite::connect_async_tls_with_config(
            url,
            Some(config),
            false,
            Some(connector),
        )
        .await
        .ok()?;
        Some(Native(ws))
    }

    impl Transport for Native {
        async fn send(&mut self, message: Vec<u8>) -> Result<(), Closed> {
            self.0
                .send(Message::Binary(message.into()))
                .await
                .map_err(|_| Closed)
        }

        async fn recv(&mut self) -> Option<Vec<u8>> {
            loop {
                match self.0.next().await {
                    Some(Ok(Message::Binary(bytes))) => return Some(bytes.to_vec()),
                    Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                    _ => return None,
                }
            }
        }
    }
}

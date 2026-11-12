//! The connection to the relay (RFC 0007, "The protocol"; NOC-175): binary
//! messages each way. A session talks to the relay only through
//! [`Transport`], so the same session runs over tokio-tungstenite natively
//! and over the browser's WebSocket on wasm32.

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

#[cfg(target_arch = "wasm32")]
pub(crate) use browser::connect;
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

#[cfg(target_arch = "wasm32")]
mod browser {
    use js_sys::{ArrayBuffer, Uint8Array};
    use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::{Closure, JsValue};
    use web_sys::{BinaryType, MessageEvent, WebSocket};

    use super::{Closed, Transport};
    use crate::sync::wire;

    /// What the browser's WebSocket told its handlers.
    enum Got {
        Open,
        Message(Vec<u8>),
        /// The connection failed or closed, or the relay sent a text
        /// message or one larger than RFC 0007's largest.
        End,
    }

    type Handler = Closure<dyn FnMut(JsValue)>;

    /// A WebSocket to the relay through the browser. Its handlers stay
    /// alive as long as it does and are taken off it when it is dropped.
    pub(crate) struct Browser {
        ws: WebSocket,
        got: UnboundedReceiver<Got>,
        ended: bool,
        _handlers: [Handler; 4],
    }

    /// A handler that sends what `read` makes of each event.
    fn handler(sender: &UnboundedSender<Got>, read: fn(JsValue) -> Got) -> Handler {
        let sender = sender.clone();
        Closure::new(move |event: JsValue| {
            let _ = sender.send(read(event));
        })
    }

    /// The bytes of a binary message; a text message, or one larger than
    /// RFC 0007's largest, ends the connection.
    fn message(event: JsValue) -> Got {
        let Ok(event) = event.dyn_into::<MessageEvent>() else {
            return Got::End;
        };
        let Ok(buffer) = event.data().dyn_into::<ArrayBuffer>() else {
            return Got::End;
        };
        if buffer.byte_length() as usize > wire::MAX_MESSAGE {
            return Got::End;
        }
        Got::Message(Uint8Array::new(&buffer).to_vec())
    }

    /// Opens a WebSocket to the relay at `url`, or `None` when it cannot
    /// be opened: the browser refuses the URL, as `ws://` from a page
    /// served over `https://`, or the connection fails. The browser
    /// verifies `wss://` against its own trust store.
    pub(crate) async fn connect(url: &str) -> Option<Browser> {
        let ws = WebSocket::new(url).ok()?;
        ws.set_binary_type(BinaryType::Arraybuffer);
        let (sender, got) = unbounded_channel();
        let handlers = [
            handler(&sender, |_| Got::Open),
            handler(&sender, message),
            handler(&sender, |_| Got::End),
            handler(&sender, |_| Got::End),
        ];
        ws.set_onopen(Some(handlers[0].as_ref().unchecked_ref()));
        ws.set_onmessage(Some(handlers[1].as_ref().unchecked_ref()));
        ws.set_onerror(Some(handlers[2].as_ref().unchecked_ref()));
        ws.set_onclose(Some(handlers[3].as_ref().unchecked_ref()));
        let mut browser = Browser {
            ws,
            got,
            ended: false,
            _handlers: handlers,
        };
        match browser.got.recv().await {
            Some(Got::Open) => Some(browser),
            _ => None,
        }
    }

    impl Transport for Browser {
        async fn send(&mut self, message: Vec<u8>) -> Result<(), Closed> {
            if self.ended || self.ws.ready_state() != WebSocket::OPEN {
                return Err(Closed);
            }
            self.ws.send_with_u8_array(&message).map_err(|_| Closed)
        }

        async fn recv(&mut self) -> Option<Vec<u8>> {
            if self.ended {
                return None;
            }
            match self.got.recv().await {
                Some(Got::Message(bytes)) => Some(bytes),
                _ => {
                    self.ended = true;
                    None
                }
            }
        }
    }

    impl Drop for Browser {
        fn drop(&mut self) {
            self.ws.set_onopen(None);
            self.ws.set_onmessage(None);
            self.ws.set_onerror(None);
            self.ws.set_onclose(None);
            let _ = self.ws.close();
        }
    }
}

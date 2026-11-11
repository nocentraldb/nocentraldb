//! A stream that first yields bytes already read from it: the head of a
//! request, read to tell a WebSocket upgrade from a plain HTTP request,
//! goes back in front of the stream for the WebSocket handshake.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;

/// `prefix`, then the rest of `inner`; writes go to `inner`.
pub(crate) struct Prefixed {
    prefix: Vec<u8>,
    read: usize,
    inner: TcpStream,
}

impl Prefixed {
    pub(crate) fn new(prefix: Vec<u8>, inner: TcpStream) -> Prefixed {
        Prefixed {
            prefix,
            read: 0,
            inner,
        }
    }
}

impl AsyncRead for Prefixed {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.read < this.prefix.len() {
            let rest = &this.prefix[this.read..];
            let n = rest.len().min(buf.remaining());
            buf.put_slice(&rest[..n]);
            this.read += n;
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut this.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for Prefixed {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

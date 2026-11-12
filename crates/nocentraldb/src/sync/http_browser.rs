//! The upload and download URLs of large changes (RFC 0007, "Large
//! changes") in the browser, through `fetch()` (NOC-175). The relay's URLs
//! answer CORS for any origin (NOC-245), so a page of any origin reaches
//! them. As strict as the native client (`http.rs`) where the browser lets
//! it be: a redirect is refused, any status other than success fails, and a
//! download is read as a stream and fails as soon as its body is longer
//! than the size the relay announced, or when it ends shorter. The browser
//! verifies `https://` against its own trust store.

use std::time::Duration;

use js_sys::{Function, Promise, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{ReadableStreamDefaultReader, Request, RequestInit, RequestRedirect, Response};

/// The largest frame the relay accepts: 16 MiB (RFC 0007, "Large
/// changes").
pub(crate) const MAX_FRAME: u64 = 16 * 1024 * 1024;

/// The longest one request may take, a 16 MiB body included.
const TRANSFER: Duration = Duration::from_secs(5 * 60);

/// Why a request failed. A failure never carries the URL, which holds a
/// token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Failed {
    /// Not an `http://` or `https://` URL.
    Unsupported,
    /// The request failed, was redirected, or took longer than
    /// [`TRANSFER`].
    Network,
    /// The server answered with this status, not success.
    Status(u16),
    /// The body is not of the size it must be, or larger than 16 MiB.
    Size,
}

/// `GET url`: a body of exactly `size` bytes.
pub(crate) async fn get(url: &str, size: u64) -> Result<Vec<u8>, Failed> {
    if size > MAX_FRAME {
        return Err(Failed::Size);
    }
    timed(async {
        let response = fetch("GET", url, None).await?;
        if response.status() != 200 {
            return Err(Failed::Status(response.status()));
        }
        let declared = response.headers().get("content-length").ok().flatten();
        if let Some(declared) = declared
            && declared.trim().parse::<u64>().ok() != Some(size)
        {
            return Err(Failed::Size);
        }
        read(&response, size).await
    })
    .await
}

/// `PUT url` with `body`.
pub(crate) async fn put(url: &str, body: &[u8]) -> Result<(), Failed> {
    timed(async {
        let body = Uint8Array::from(body);
        let response = fetch("PUT", url, Some(&body)).await?;
        match response.status() {
            200..=299 => Ok(()),
            status => Err(Failed::Status(status)),
        }
    })
    .await
}

async fn timed<T>(
    request: impl std::future::Future<Output = Result<T, Failed>>,
) -> Result<T, Failed> {
    super::time::timeout(TRANSFER, request)
        .await
        .unwrap_or(Err(Failed::Network))
}

/// `fetch(url)` with `method` and `body`, a redirect refused, through the
/// global's `fetch`, which a window and a worker both have.
async fn fetch(method: &str, url: &str, body: Option<&Uint8Array>) -> Result<Response, Failed> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(Failed::Unsupported);
    }
    let init = RequestInit::new();
    init.set_method(method);
    init.set_redirect(RequestRedirect::Error);
    if let Some(body) = body {
        init.set_body(body);
    }
    let request = Request::new_with_str_and_init(url, &init).map_err(|_| Failed::Unsupported)?;
    let global = js_sys::global();
    let fetch = Reflect::get(&global, &JsValue::from_str("fetch"))
        .ok()
        .and_then(|fetch| fetch.dyn_into::<Function>().ok())
        .ok_or(Failed::Network)?;
    let promise = fetch
        .call1(&global, &request)
        .ok()
        .and_then(|promise| promise.dyn_into::<Promise>().ok())
        .ok_or(Failed::Network)?;
    JsFuture::from(promise)
        .await
        .ok()
        .and_then(|response| response.dyn_into::<Response>().ok())
        .ok_or(Failed::Network)
}

/// The body of `response`, read chunk by chunk: exactly `size` bytes, or
/// [`Failed::Size`] as soon as it is longer, or when it ends shorter.
async fn read(response: &Response, size: u64) -> Result<Vec<u8>, Failed> {
    let Some(stream) = response.body() else {
        return if size == 0 {
            Ok(Vec::new())
        } else {
            Err(Failed::Size)
        };
    };
    let reader = stream
        .get_reader()
        .dyn_into::<ReadableStreamDefaultReader>()
        .map_err(|_| Failed::Network)?;
    let mut body = Vec::with_capacity(size as usize);
    loop {
        let chunk = JsFuture::from(reader.read())
            .await
            .map_err(|_| Failed::Network)?;
        let done = Reflect::get(&chunk, &JsValue::from_str("done"))
            .ok()
            .and_then(|done| done.as_bool())
            .ok_or(Failed::Network)?;
        if done {
            break;
        }
        let value = Reflect::get(&chunk, &JsValue::from_str("value"))
            .ok()
            .and_then(|value| value.dyn_into::<Uint8Array>().ok())
            .ok_or(Failed::Network)?;
        if body.len() as u64 + u64::from(value.length()) > size {
            let _ = reader.cancel();
            return Err(Failed::Size);
        }
        let start = body.len();
        body.resize(start + value.length() as usize, 0);
        value.copy_to(&mut body[start..]);
    }
    if body.len() as u64 != size {
        return Err(Failed::Size);
    }
    Ok(body)
}

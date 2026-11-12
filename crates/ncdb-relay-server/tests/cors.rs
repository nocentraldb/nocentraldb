//! RFC 0007, "Large changes", as NOC-245 recommends (1A, for the owner's
//! answer): the upload and download URLs answer CORS for any origin, so
//! that an app in a browser reaches them with `fetch()`. A URL is a one-time capability and
//! carries no cookie, so any origin gains nothing by it. A `PUT` asks the
//! browser's preflight first, which the server answers for both paths.

mod support;

use support::http::{self, Response};
use support::*;

/// A token in hex no upload or download holds.
const TOKEN: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

/// A page's origin, as a browser sends it.
const ORIGIN: &str = "https://app.example";

async fn preflight(url: &str, method: &str) -> Response {
    let headers = [
        ("Origin", ORIGIN),
        ("Access-Control-Request-Method", method),
        ("Access-Control-Request-Headers", "content-type"),
    ];
    http::request_with_headers("OPTIONS", url, &headers, 0, &[]).await
}

/// Whether the comma-separated `list` names `item`, in any case.
fn names(list: Option<&str>, item: &str) -> bool {
    list.unwrap_or("")
        .split(',')
        .any(|entry| entry.trim().eq_ignore_ascii_case(item))
}

#[tokio::test]
async fn the_preflight_of_an_upload_url_allows_any_origin_to_put() {
    let (addr, _dir) = server().await;
    let url = format!("http://{addr}/uploads/{TOKEN}");
    let response = preflight(&url, "PUT").await;
    assert_eq!(response.status, 204, "{response:?}");
    assert_eq!(response.header("access-control-allow-origin"), Some("*"));
    let methods = response.header("access-control-allow-methods");
    assert!(names(methods, "PUT"), "{methods:?}");
    let headers = response.header("access-control-allow-headers");
    assert!(names(headers, "content-type"), "{headers:?}");
    assert!(response.header("access-control-max-age").is_some());
}

#[tokio::test]
async fn the_preflight_of_a_download_url_allows_any_origin_to_get() {
    let (addr, _dir) = server().await;
    let url = format!("http://{addr}/frames/{TOKEN}");
    let response = preflight(&url, "GET").await;
    assert_eq!(response.status, 204, "{response:?}");
    assert_eq!(response.header("access-control-allow-origin"), Some("*"));
    let methods = response.header("access-control-allow-methods");
    assert!(names(methods, "GET"), "{methods:?}");
}

#[tokio::test]
async fn an_answer_of_an_upload_or_download_url_allows_any_origin_to_read_it() {
    let (addr, _dir) = server().await;
    let origin = [("Origin", ORIGIN)];
    let download = format!("http://{addr}/frames/{TOKEN}");
    let got = http::request_with_headers("GET", &download, &origin, 0, &[]).await;
    assert_eq!(got.status, 404);
    assert_eq!(got.header("access-control-allow-origin"), Some("*"));

    let upload = format!("http://{addr}/uploads/{TOKEN}");
    let put = http::request_with_headers("PUT", &upload, &origin, 4, b"abcd").await;
    assert_eq!(put.status, 404);
    assert_eq!(put.header("access-control-allow-origin"), Some("*"));
}

#[tokio::test]
async fn a_preflight_of_any_other_path_is_not_found() {
    let (addr, _dir) = server().await;
    let response = preflight(&format!("http://{addr}/other"), "PUT").await;
    assert_eq!(response.status, 404);
    let bad_token = preflight(&format!("http://{addr}/uploads/xyz"), "PUT").await;
    assert_eq!(bad_token.status, 404);
}

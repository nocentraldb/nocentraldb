//! The URLs of large changes on AWS are presigned S3 URLs (RFC 0007, "How
//! large changes go to S3"): an upload's goes to `uploads/<upload id>` for
//! 15 minutes with its size fixed in the signature, an entry's comes from
//! `frames/<space id>/<change id>` for 5 minutes. Presigning needs no
//! network, so these tests run anywhere.

use std::collections::BTreeSet;

use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use ncdb_core::change::Kind;
use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::store::{Entry, Token, UploadId};
use ncdb_relay::{Config, Relay};
use ncdb_relay_lambda::Urls;
use ncdb_relay_lambda::s3::S3Urls;

fn urls() -> S3Urls {
    let config = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new("eu-central-1"))
        .credentials_provider(Credentials::new(
            "AKIDEXAMPLE",
            "secret",
            None,
            None,
            "test",
        ))
        .build();
    S3Urls::new(aws_sdk_s3::Client::from_conf(config), "ncdb-frames")
}

/// A query parameter of a URL.
fn param<'a>(url: &'a str, name: &str) -> Option<&'a str> {
    let query = url.split_once('?')?.1;
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix(&format!("{name}=")))
}

#[tokio::test]
async fn an_upload_url_puts_its_size_to_its_key_for_15_minutes() {
    let id = UploadId([0xab; 16]);
    let url = Urls::<SqliteStore>::upload(&urls(), id, Token([1; 32]), 200_000)
        .await
        .unwrap();
    assert!(
        url.starts_with(&format!(
            "https://ncdb-frames.s3.eu-central-1.amazonaws.com/uploads/{}?",
            "ab".repeat(16)
        )),
        "{url}"
    );
    assert_eq!(param(&url, "X-Amz-Expires"), Some("900"));
    let signed = param(&url, "X-Amz-SignedHeaders").unwrap();
    assert!(
        signed.split("%3B").any(|h| h == "content-length"),
        "{signed}"
    );
    // The token is the relay's own, for its own listener: it never goes to S3.
    assert!(!url.contains(&"01".repeat(32)));
}

#[tokio::test]
async fn a_download_url_gets_its_frame_for_5_minutes() {
    let dir = std::env::temp_dir().join(format!("ncdb-s3-urls-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let relay = Relay::new(
        SqliteStore::open(&dir.join("relay.sqlite")).unwrap(),
        Config::new(BTreeSet::new()),
    );
    let space = SpaceId([0x0c; 16]);
    let entry = Entry {
        position: 7,
        change: ChangeId([0x0d; 32]),
        accepted_at: 1,
        author: MemberId([0x0e; 32]),
        kind: Kind::SEALED,
        frame: vec![0; 10],
    };
    let url = urls().download(&relay, space, &entry).await.unwrap();
    let key = format!("frames/{}/{}", "0c".repeat(16), "0d".repeat(32));
    assert!(
        url.starts_with(&format!(
            "https://ncdb-frames.s3.eu-central-1.amazonaws.com/{key}?"
        )),
        "{url}"
    );
    assert_eq!(param(&url, "X-Amz-Expires"), Some("300"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The function's own S3 client (`s3::client`), configured as Lambda
/// configures it: a region and a role's session credentials.
fn function_urls() -> S3Urls {
    let aws = aws_config::SdkConfig::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new("eu-central-1"))
        .credentials_provider(aws_sdk_s3::config::SharedCredentialsProvider::new(
            Credentials::new(
                "ASIAEXAMPLE",
                "secret",
                Some("session".into()),
                None,
                "test",
            ),
        ))
        .build();
    S3Urls::new(ncdb_relay_lambda::s3::client(&aws), "ncdb-frames")
}

/// S3 checks a presigned PUT against what its query parameters sign
/// (NOC-348). The URL signs only the size and the host, as the URL names
/// it without a port, and carries no checksum, which the emulator would
/// not check: a checksum of any other body than the member's frame would
/// make S3 refuse the upload.
#[tokio::test]
async fn the_functions_upload_url_signs_only_size_and_host_and_no_checksum() {
    let url = Urls::<SqliteStore>::upload(
        &function_urls(),
        UploadId([0xab; 16]),
        Token([1; 32]),
        30_000,
    )
    .await
    .unwrap();
    let host = url
        .strip_prefix("https://")
        .unwrap()
        .split('/')
        .next()
        .unwrap();
    assert_eq!(host, "ncdb-frames.s3.eu-central-1.amazonaws.com");
    assert_eq!(
        param(&url, "X-Amz-SignedHeaders"),
        Some("content-length%3Bhost")
    );
    let query = url.split_once('?').unwrap().1;
    for pair in query.split('&') {
        let name = pair.split('=').next().unwrap().to_ascii_lowercase();
        assert!(
            !name.starts_with("x-amz-checksum") && name != "x-amz-sdk-checksum-algorithm",
            "{url}"
        );
    }
}

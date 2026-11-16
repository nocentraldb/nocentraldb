//! RFC 0007, "The errors", for what this server does not read: messages it
//! cannot decode, refused with one of the RFC's codes.

mod support;

use ncdb_core::cbor::Value;
use support::space::Space;
use support::*;

#[tokio::test(flavor = "multi_thread")]
async fn a_message_the_server_cannot_read_is_malformed_and_the_connection_stays() {
    let (addr, _dir) = server().await;
    let (mut client, _) = Client::member(addr, 0).await;
    let unknown = message("gossip", Some(3), &[]);
    assert_eq!(client.ask(&unknown).await, error(Some(3), "malformed"));
    let extra = message("ping", Some(4), &[(9, Value::Unsigned(1))]);
    assert_eq!(client.ask(&extra).await, error(Some(4), "malformed"));
    // 1 written as two bytes is not canonical CBOR.
    client.send_bytes(vec![0xa1, 0x18, 0x01, 0x01]).await;
    assert_eq!(client.recv().await, error(None, "malformed"));
    let pong = client.ask(&message("ping", Some(5), &[])).await;
    assert_eq!(pong, message("pong", Some(5), &[]));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_message_above_23_kib_closes_the_connection() {
    let (addr, _dir) = server().await;
    let (mut client, _) = Client::member(addr, 0).await;
    let space = Space::new(0);
    let frames = Value::Array(vec![Value::Bytes(vec![0; 23 * 1024])]);
    let push = message(
        "push",
        Some(1),
        &[(3, Value::Bytes(space.space.0.to_vec())), (4, frames)],
    );
    client.send(&push).await;
    client.closed().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_binary_frame_is_no_message() {
    let (addr, _dir) = server().await;
    let (mut client, _) = Client::member(addr, 0).await;
    // A member's push as one binary frame, as members sent before NOC-338.
    client
        .send_binary(ncdb_core::cbor::encode(&message("ping", Some(1), &[])))
        .await;
    let error = client.recv().await;
    assert_eq!(kind(&error), "error");
    assert_eq!(text(field(&error, 3)), "malformed");
}

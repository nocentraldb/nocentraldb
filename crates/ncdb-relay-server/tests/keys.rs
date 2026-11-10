//! RFC 0007, "Keys and the person-key store", over a real socket: `keys`
//! answers a member's key envelopes and the key chain, `put-person-keys`
//! stores records the relay core accepts and answers `ok`, and
//! `person-keys` reads them back.

mod support;

use ncdb_core::cbor::Value;
use ncdb_core::change::Change;
use ncdb_core::ids::SpaceId;
use ncdb_core::membership::{self, Role};
use support::space::{Space, id, key, person, wrapped};
use support::*;

const CREATOR: u8 = 0;
const WRITER: u8 = 1;
const STRANGER: u8 = 9;

fn space_value(space: SpaceId) -> Value {
    Value::Bytes(space.0.to_vec())
}

/// A record of person 1 under `generation`, by `writer`: `{1: person-key
/// id, 2: generation, 3: writer member id, 4: wrapped, 5: signature}`.
fn record(space: SpaceId, writer: u8, generation: u64) -> Value {
    let wrapped = wrapped(1, generation);
    let signature = membership::sign_record(&key(writer), space, person(1), generation, &wrapped);
    map(&[
        (1, Value::Bytes(person(1).to_vec())),
        (2, Value::Unsigned(generation)),
        (3, Value::Bytes(id(writer).0.to_vec())),
        (4, Value::Bytes(wrapped)),
        (5, Value::Bytes(signature.to_vec())),
    ])
}

fn map(fields: &[(u64, Value)]) -> Value {
    let Value::Map(mut entries) = message("x", None, fields) else {
        unreachable!("a message is a map")
    };
    entries.remove(&ncdb_core::cbor::Key::Unsigned(1));
    Value::Map(entries)
}

fn put(req: u64, space: SpaceId, records: Vec<Value>) -> Value {
    message(
        "put-person-keys",
        Some(req),
        &[(3, space_value(space)), (4, Value::Array(records))],
    )
}

/// The creator's space with a writer, who created person 1's key.
async fn held(client: &mut Client) -> (Space, Vec<Change>) {
    let mut space = Space::new(CREATOR);
    let genesis = space.genesis.clone();
    let writer = space.add(CREATOR, WRITER, Role::Writer, &[&genesis]);
    let entry = space.person_key(WRITER, 1, 0xc1, &[&writer]);
    let results = client
        .push(1, space.space, &[&genesis, &writer, &entry])
        .await;
    assert_eq!(results, [accepted(1), accepted(2), accepted(3)]);
    (space, vec![genesis, writer, entry])
}

#[tokio::test(flavor = "multi_thread")]
async fn keys_answers_the_members_envelopes_and_the_key_chain() {
    let (addr, _dir) = server().await;
    let (mut creator, _) = Client::member(addr, CREATOR).await;
    let (mut space, held) = held(&mut creator).await;
    let to_writer = space.envelope(CREATOR, WRITER, &[&held[2]]);
    let to_creator = space.envelope(CREATOR, CREATOR, &[&to_writer]);
    let (chain, _) = space.rotate(CREATOR, &[space.first_generation], &[&to_creator]);
    let results = creator
        .push(2, space.space, &[&to_writer, &to_creator, &chain])
        .await;
    assert_eq!(results, [accepted(4), accepted(5), accepted(6)]);

    let (mut writer, _) = Client::member(addr, WRITER).await;
    let keys = writer
        .ask(&message("keys", Some(7), &[(3, space_value(space.space))]))
        .await;
    assert_eq!(kind(&keys), "keys", "{keys:?}");
    assert_eq!(support::keys(&keys), [1, 2, 3, 4, 5]);
    assert_eq!(uint(field(&keys, 2)), 7);
    assert_eq!(bytes(field(&keys, 3)), space.space.0);
    let frames = |k: u64| -> Vec<(u64, Vec<u8>)> {
        array(field(&keys, k))
            .iter()
            .map(|entry| {
                assert_eq!(support::keys(entry), [1, 2, 3]);
                (uint(field(entry, 1)), bytes(field(entry, 3)).to_vec())
            })
            .collect()
    };
    assert_eq!(frames(4), [(4, to_writer.bytes().to_vec())]);
    assert_eq!(frames(5), [(6, chain.bytes().to_vec())]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_record_is_put_then_read_back() {
    let (addr, _dir) = server().await;
    let (mut writer, _) = Client::member(addr, CREATOR).await;
    let (space, _) = held(&mut writer).await;
    let record = record(space.space, WRITER, space.first_generation);
    let ok = writer.ask(&put(2, space.space, vec![record.clone()])).await;
    assert_eq!(ok, message("ok", Some(2), &[]));

    let (mut other, _) = Client::member(addr, WRITER).await;
    let read = message("person-keys", Some(3), &[(3, space_value(space.space))]);
    let records = other.ask(&read).await;
    assert_eq!(
        records,
        message(
            "person-keys",
            Some(3),
            &[
                (3, space_value(space.space)),
                (4, Value::Array(vec![record]))
            ]
        )
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bad_record_is_refused_and_a_malformed_one_is_malformed() {
    let (addr, _dir) = server().await;
    let (mut writer, _) = Client::member(addr, CREATOR).await;
    let (space, _) = held(&mut writer).await;
    // A generation no person-key entry announces.
    let unannounced = record(space.space, WRITER, space.first_generation + 1);
    let refused = writer.ask(&put(2, space.space, vec![unannounced])).await;
    assert_eq!(refused, error(Some(2), "bad_record"));

    let Value::Map(mut short) = record(space.space, WRITER, space.first_generation) else {
        unreachable!()
    };
    short.insert(ncdb_core::cbor::Key::Unsigned(1), Value::Bytes(vec![1; 15]));
    let malformed = writer
        .ask(&put(3, space.space, vec![Value::Map(short)]))
        .await;
    assert_eq!(malformed, error(Some(3), "malformed"));
}

#[tokio::test(flavor = "multi_thread")]
async fn only_current_members_read_keys_and_records() {
    let (addr, _dir) = server().await;
    let (mut creator, _) = Client::member(addr, CREATOR).await;
    let (space, _) = held(&mut creator).await;
    let (mut stranger, _) = Client::member(addr, STRANGER).await;
    let target = space_value(space.space);
    let requests = [
        message("keys", Some(1), &[(3, target.clone())]),
        message("person-keys", Some(2), &[(3, target)]),
        put(
            3,
            space.space,
            vec![record(space.space, WRITER, space.first_generation)],
        ),
    ];
    for (n, request) in requests.iter().enumerate() {
        let req = n as u64 + 1;
        assert_eq!(stranger.ask(request).await, error(Some(req), "not_member"));
    }
}

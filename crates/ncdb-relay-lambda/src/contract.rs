//! What every [`Connections`] store does, as one test that runs against
//! each: [`MemoryConnections`](crate::MemoryConnections) in the crate's
//! own tests, the DynamoDB store against the AWS Emulator. A store is
//! interchangeable when it passes.
//!
//! The rules: a stored connection reads back as stored, and is gone once
//! its record expired; a subscription lists under its space; a connection
//! that acts as a member lists under that member; `remove` takes the
//! connection and all its subscriptions; and `subscribe` again replaces.

use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_relay::store::Epoch;

use crate::connections::{Connection, Connections, Stage, Subscription};

/// Milliseconds since the Unix epoch.
pub fn now() -> u64 {
    let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    since.map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn open(stage: Stage) -> Connection {
    let at = now();
    Connection {
        stage,
        connected_at: at,
        expires_at: at + crate::connections::MAX_CONNECTION_AGE,
    }
}

fn sorted<T: Ord>(mut v: Vec<T>) -> Vec<T> {
    v.sort();
    v
}

/// Runs the contract against `store`, whose ids it prefixes with `tag`,
/// so runs against one shared store do not meet. Panics on a broken rule.
pub async fn run<C: Connections>(store: &C, tag: &str) {
    let id = |n: u8| format!("{tag}-c{n}");
    let member = MemberId([0x11; 32]);
    let other = MemberId([0x22; 32]);
    let space = SpaceId([0x33; 16]);
    let elsewhere = SpaceId([0x44; 16]);
    let epoch = Epoch([0x55; 16]);

    // A connection reads back as stored, through each step of its opening.
    assert_eq!(store.get(&id(1)).await.unwrap(), None, "an unknown id");
    let hello = open(Stage::Hello);
    store.put(&id(1), hello).await.unwrap();
    assert_eq!(store.get(&id(1)).await.unwrap(), Some(hello));
    let auth = Connection {
        stage: Stage::Auth { nonce: [7; 32] },
        ..hello
    };
    store.put(&id(1), auth).await.unwrap();
    assert_eq!(store.get(&id(1)).await.unwrap(), Some(auth));
    let acting = Connection {
        stage: Stage::Member(member),
        ..hello
    };
    store.put(&id(1), acting).await.unwrap();
    assert_eq!(store.get(&id(1)).await.unwrap(), Some(acting));

    // The connections of a member: only those that act as it.
    store
        .put(&id(2), open(Stage::Member(member)))
        .await
        .unwrap();
    store.put(&id(3), open(Stage::Member(other))).await.unwrap();
    store.put(&id(4), open(Stage::Hello)).await.unwrap();
    let mine = sorted(store.of_member(member).await.unwrap());
    let mine: Vec<String> = mine.into_iter().filter(|c| c.starts_with(tag)).collect();
    assert_eq!(mine, [id(1), id(2)]);

    // Subscriptions list under their space, with their request id and
    // epoch; subscribing again replaces.
    let sub = |req| Subscription { member, req, epoch };
    store.subscribe(&id(1), space, sub(5)).await.unwrap();
    store.subscribe(&id(1), space, sub(6)).await.unwrap();
    store.subscribe(&id(1), elsewhere, sub(7)).await.unwrap();
    store
        .subscribe(
            &id(3),
            space,
            Subscription {
                member: other,
                req: 8,
                epoch,
            },
        )
        .await
        .unwrap();
    let listed = |found: Vec<(String, Subscription)>| -> Vec<(String, Subscription)> {
        let mut found: Vec<_> = found
            .into_iter()
            .filter(|(c, _)| c.starts_with(tag))
            .collect();
        found.sort_by(|a, b| a.0.cmp(&b.0));
        found
    };
    assert_eq!(
        listed(store.subscribers(space).await.unwrap()),
        [
            (id(1), sub(6)),
            (
                id(3),
                Subscription {
                    member: other,
                    req: 8,
                    epoch
                }
            )
        ]
    );
    store.unsubscribe(&id(3), space).await.unwrap();
    assert_eq!(
        listed(store.subscribers(space).await.unwrap()),
        [(id(1), sub(6))]
    );

    // Removing a connection takes its subscriptions with it.
    store.remove(&id(1)).await.unwrap();
    assert_eq!(store.get(&id(1)).await.unwrap(), None);
    assert!(listed(store.subscribers(space).await.unwrap()).is_empty());
    assert!(listed(store.subscribers(elsewhere).await.unwrap()).is_empty());
    let mine: Vec<String> = store.of_member(member).await.unwrap();
    assert_eq!(
        sorted(
            mine.into_iter()
                .filter(|c| c.starts_with(tag))
                .collect::<Vec<_>>()
        ),
        [id(2)]
    );
    // Removing an unknown connection is no error.
    store.remove(&id(9)).await.unwrap();

    // A record whose time ran out is gone, as a missed `$disconnect`
    // leaves it until the TTL deletes it.
    let expired = Connection {
        stage: Stage::Member(member),
        connected_at: 1,
        expires_at: 2,
    };
    store.put(&id(5), expired).await.unwrap();
    store.subscribe(&id(5), space, sub(9)).await.unwrap();
    assert_eq!(store.get(&id(5)).await.unwrap(), None);
    assert!(listed(store.subscribers(space).await.unwrap()).is_empty());
    assert!(!store.of_member(member).await.unwrap().contains(&id(5)));

    for n in 1..=5 {
        store.remove(&id(n)).await.unwrap();
    }
}

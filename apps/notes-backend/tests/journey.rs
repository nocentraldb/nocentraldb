//! RFC 0015's "A user's journey", run end to end: the example backend
//! against the test relay, which takes only admissions the backend's
//! admission key signed, with each user's device on the public Rust SDK.
//!
//! Ana signs up, and her device's first space is admitted through the
//! backend. She keeps a personal space and a project, each marked with a
//! `spaceInfo` object of the app's model. She invites Ben before he has an
//! account; the backend keeps the invitation pending until Ben signs up,
//! then Ana's device adds him. Ben's device tells the project from his
//! personal space by its `spaceInfo`, and never lists the account space.
//! Last, Ana shares a single note with Ben by moving it into a space of its
//! own.

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use common::{next, request, sign_up, synced};
use ed25519_dalek::SigningKey;
use ncdb_relay_server::local::{Options, TestRelay};
use nocentraldb::{AddMember, Db, Invitation, OpenOptions, Role, Space, SpaceId, Value};
use notes_backend::{Backend, Config};

/// The app's model, `apps/notes-backend/model.ts`.
const LOCK: &str = include_str!("../model.lock.json");

/// A device of the user with `session`, in memory, that syncs through
/// `relay` and fetches its admissions from the backend at `address`, as
/// the app's `admission` function does. The app then tells its backend the
/// device's invitation, so that others find it in the directory.
async fn open(relay: &TestRelay, address: &str, session: &str) -> Db {
    let (to, from) = (address.to_owned(), session.to_owned());
    let options = OpenOptions::new(LOCK)
        .in_memory()
        .relay(&relay.url())
        .admission(move |identity: String| {
            let answer = request(&to, "POST", "/admission", Some(&from), &identity);
            async move {
                match answer.status {
                    200 => Ok(answer.body),
                    status => Err(status),
                }
            }
        });
    let db = Db::open(options).await.expect("the database opens");
    let invitation = db.invitation().await.to_string();
    let answer = request(
        address,
        "PUT",
        "/me/invitations",
        Some(session),
        &invitation,
    );
    assert_eq!(answer.status, 204, "{}", answer.body);
    db
}

/// Creates a space of `kind` named `name`: the space, and in it the
/// `spaceInfo` object that says what it is.
async fn create(db: &Db, kind: &str, name: &str) -> Space {
    let space = db.spaces().create().await.expect("a space");
    space
        .create(
            "spaceInfo",
            [("kind", Value::from(kind)), ("name", name.into())],
        )
        .await
        .expect("its spaceInfo");
    space
}

/// The kind and name of each app space `db` holds, by space: what the
/// `spaceInfo` objects of its spaces say.
async fn kinds(db: &Db) -> BTreeMap<SpaceId, (String, String)> {
    let query = db
        .index("spaceInfo", "byKind")
        .expect("the index")
        .take(100);
    let page = db.find(query).await.expect("a page");
    page.items
        .iter()
        .map(|info| {
            let kind = info.get::<&str>("kind").unwrap().unwrap_or("").to_owned();
            let name = info.get::<&str>("name").unwrap().unwrap_or("").to_owned();
            (info.space(), (kind, name))
        })
        .collect()
}

/// Waits until `db` lists `space` and holds its `spaceInfo`.
async fn arrives(db: &Db, space: SpaceId) {
    let mut lists = db.spaces().subscribe();
    while !next(&mut lists).await.iter().any(|s| s.id == space) {}
    let deadline = tokio::time::Instant::now() + common::WAIT;
    while !kinds(db).await.contains_key(&space) {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the spaceInfo arrives"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn invitations_flow_through_the_backend_and_spaces_tell_their_kind() {
    // The operator configures the public half of the app's admission key
    // on the relay, NCDB_ADMISSION_KEYS; the backend holds the secret.
    let key = SigningKey::from_bytes(&[0x42; 32]);
    let relay = TestRelay::start(Options::new().admission_key(key.verifying_key().to_bytes()))
        .expect("the relay starts");
    let backend = Backend::start(Config::new(relay.relay_id(), key)).expect("the backend starts");
    let at = backend.address();

    // Ana signs up; her first space is admitted through the backend.
    let ana_session = sign_up(&at, "ana@example.com");
    let ana = open(&relay, &at, &ana_session).await;
    let personal = create(&ana, "personal", "Ana").await;
    synced(&ana).await;
    let me = request(
        &at,
        "GET",
        "/users/ana@example.com/invitations",
        Some(&ana_session),
        "",
    );
    assert_eq!(me.status, 200, "{}", me.body);
    assert_eq!(
        me.json()["identity"].as_str(),
        ana.account().identity().as_deref(),
        "the directory holds the identity it admitted"
    );

    // Her project, and a note in it.
    let project = create(&ana, "project", "Launch").await;
    project
        .create("notes", [("title", Value::from("Plan"))])
        .await
        .expect("a note");
    synced(&ana).await;

    // She invites Ben, who has no account yet: the invitation waits.
    let invite = serde_json::json!({ "email": "ben@example.com", "space": project.id().to_string(), "role": "writer" });
    let pending = request(
        &at,
        "POST",
        "/invites",
        Some(&ana_session),
        &invite.to_string(),
    );
    assert_eq!(pending.status, 200, "{}", pending.body);
    assert_eq!(pending.json()["ready"], false);
    let ready = request(&at, "GET", "/invites/ready", Some(&ana_session), "");
    assert_eq!(ready.json()["invites"].as_array().map(Vec::len), Some(0));

    // Ben signs up and opens the app: his device's invitation reaches the
    // directory, and his personal space is admitted.
    let ben_session = sign_up(&at, "ben@example.com");
    let ben = open(&relay, &at, &ben_session).await;
    let bens = create(&ben, "personal", "Ben").await;
    synced(&ben).await;

    // Ana's device learns that the invitation can be carried out, and adds
    // each of Ben's devices with the fingerprint the directory reports.
    let ready = request(&at, "GET", "/invites/ready", Some(&ana_session), "");
    let invites = ready.json()["invites"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(invites.len(), 1, "{}", ready.body);
    assert_eq!(
        invites[0]["space"].as_str(),
        Some(project.id().to_string().as_str())
    );
    for text in invites[0]["invitations"].as_array().unwrap() {
        let invitation: Invitation = text.as_str().unwrap().parse().expect("an invitation");
        let fingerprint = Some(invitation.fingerprint());
        project
            .members()
            .add(
                &invitation,
                AddMember {
                    role: Role::Writer,
                    fingerprint,
                },
            )
            .await
            .expect("Ben is added");
    }
    let id = invites[0]["id"].as_str().unwrap();
    let done = request(
        &at,
        "POST",
        &format!("/invites/{id}/done"),
        Some(&ana_session),
        "",
    );
    assert_eq!(done.status, 204, "{}", done.body);
    synced(&ana).await;

    // Ben's device sees the project, and tells it from his personal space
    // by its spaceInfo. The account space is the SDK's: never listed.
    arrives(&ben, project.id()).await;
    let seen = kinds(&ben).await;
    assert_eq!(
        seen.get(&project.id()),
        Some(&("project".to_owned(), "Launch".to_owned()))
    );
    assert_eq!(
        seen.get(&bens.id()),
        Some(&("personal".to_owned(), "Ben".to_owned()))
    );
    let listed: Vec<SpaceId> = ben
        .spaces()
        .list()
        .await
        .unwrap()
        .iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(
        listed.len(),
        2,
        "Ben's personal space and the project, no account space"
    );

    // Ana shares one note of her personal space with Ben alone: a space
    // for the note, Ben added (his account is known now), then the move.
    let note = personal
        .create("notes", [("title", Value::from("For Ben"))])
        .await
        .expect("a note")
        .ids[0];
    let shared = create(&ana, "note", "For Ben").await;
    for text in me_invitations(&at, &ana_session, "ben@example.com") {
        let invitation: Invitation = text.parse().unwrap();
        let options = AddMember {
            role: Role::Writer,
            fingerprint: None,
        };
        shared
            .members()
            .add(&invitation, options)
            .await
            .expect("Ben is added");
    }
    let object = personal
        .get("notes", note)
        .await
        .unwrap()
        .expect("the note");
    personal
        .move_objects(&[object], &shared)
        .await
        .expect("the note moves");
    synced(&ana).await;

    arrives(&ben, shared.id()).await;
    let deadline = tokio::time::Instant::now() + common::WAIT;
    loop {
        let space = ben
            .spaces()
            .get(shared.id())
            .await
            .expect("the shared space");
        if let Some(read) = space.get("notes", note).await.unwrap() {
            assert_eq!(read.get::<&str>("title").unwrap(), Some("For Ben"));
            break;
        }
        assert!(tokio::time::Instant::now() < deadline, "the note arrives");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        !kinds(&ben).await.contains_key(&personal.id()),
        "Ana's personal space stays hers"
    );
}

/// The invitations the directory holds for `email`.
fn me_invitations(at: &str, session: &str, email: &str) -> Vec<String> {
    let answer = request(
        at,
        "GET",
        &format!("/users/{email}/invitations"),
        Some(session),
        "",
    );
    assert_eq!(answer.status, 200, "{}", answer.body);
    answer.json()["invitations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i.as_str().unwrap().to_owned())
        .collect()
}

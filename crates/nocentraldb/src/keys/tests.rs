//! The key record, and the keys in the platform keystore (RFC 0003, "The
//! member key"; RFC 0009, "The Rust SDK" and "Open and close"). A fake keystore stands in for the
//! platform's, so that CI, which has none, tests every branch; the one test
//! of the platform's own keystore runs where one is reachable.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use zeroize::Zeroizing;

use super::store::{self, SecretStore};
use super::*;
use crate::{Db, OpenOptions};

const LOCK: &str = include_str!("../../tests/model/model.lock.json");

#[test]
fn a_key_record_reads_back_as_the_same_keys() {
    let keys = Keys::generate();

    let read = Keys::decode(&keys.encode()).unwrap();

    assert_eq!(read.member.key().id(), keys.member.key().id());
    assert_eq!(read.account.to_bytes(), keys.account.to_bytes());
    assert_eq!(keys.encode().len(), LEN);
}

#[test]
fn a_record_of_another_format_or_length_is_not_read() {
    let mut record = Keys::generate().encode().to_vec();
    assert!(Keys::decode(&record[..LEN - 1]).is_none());
    record[MAGIC.len()] = 2;
    assert!(Keys::decode(&record).is_none());
}

/// A keystore in memory, shared by the databases a test opens.
#[derive(Clone, Default)]
struct Fake(Arc<Mutex<BTreeMap<String, Vec<u8>>>>);

impl SecretStore for Fake {
    fn get(&self, user: &str) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        let entries = self.0.lock().unwrap();
        Ok(entries
            .get(user)
            .map(|record| Zeroizing::new(record.clone())))
    }

    fn set(&self, user: &str, record: &[u8]) -> Result<(), Error> {
        self.0
            .lock()
            .unwrap()
            .insert(user.to_owned(), record.to_vec());
        Ok(())
    }

    fn delete(&self, user: &str) -> Result<(), Error> {
        self.0.lock().unwrap().remove(user);
        Ok(())
    }
}

/// A directory of its own, removed when dropped.
struct Dir(std::path::PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let unique = format!("nocentraldb-keys-{name}-{}", hex(&*random()));
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).unwrap();
        Dir(dir)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn run<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("the SDK never waits on the device"),
    }
}

/// No keystore, as on a machine without a Secret Service.
fn unavailable() -> Result<Box<dyn SecretStore>, keyring_core::Error> {
    Err(keyring_core::Error::NoDefaultStore)
}

#[test]
fn without_a_keystore_or_a_key_file_opening_names_the_key_file_option() {
    let dir = Dir::new("none");
    let data = dir.0.join("data");

    let error = Db::open_with_store(OpenOptions::new(LOCK).path(&data), &unavailable).unwrap_err();

    assert_eq!(error.code(), ErrorCode::KeyStoreUnavailable);
    assert!(error.to_string().contains("key_file"), "{error}");
    assert!(!data.exists(), "nothing lands on disk");
}

#[test]
fn a_reopened_database_finds_its_keys_in_the_keystore() {
    let dir = Dir::new("reopen");
    let fake = Fake::default();
    let store = || Ok(Box::new(fake.clone()) as Box<dyn SecretStore>);
    let options = || OpenOptions::new(LOCK).path(dir.0.join("data"));
    let space = {
        let db = Db::open_with_store(options(), &store).unwrap();
        run(db.spaces().create()).unwrap().id()
    };

    let db = Db::open_with_store(options(), &store).unwrap();
    let project = run(db.spaces().get(space)).unwrap();

    run(project.write(|tx| {
        tx.create("tasks", [("title", crate::Value::from("again"))])?;
        Ok(())
    }))
    .unwrap();
    let entries = fake.0.lock().unwrap();
    let canonical = dir.0.join("data").canonicalize().unwrap();
    assert_eq!(
        entries.keys().collect::<Vec<_>>(),
        [&canonical.to_string_lossy().into_owned()]
    );
}

#[test]
fn an_existing_database_whose_keystore_entry_is_gone_does_not_open() {
    let dir = Dir::new("gone");
    let options = || OpenOptions::new(LOCK).path(dir.0.join("data"));
    let first = Fake::default();
    drop(
        Db::open_with_store(options(), &|| {
            Ok(Box::new(first.clone()) as Box<dyn SecretStore>)
        })
        .unwrap(),
    );

    let empty = Fake::default();
    let store = || Ok(Box::new(empty.clone()) as Box<dyn SecretStore>);
    let error = Db::open_with_store(options(), &store).unwrap_err();

    assert_eq!(error.code(), ErrorCode::StorageFailed);
    assert!(empty.0.lock().unwrap().is_empty(), "no new key is drawn");
}

#[test]
fn a_key_file_is_used_instead_of_the_keystore() {
    let dir = Dir::new("file");
    let fake = Fake::default();
    let store = || Ok(Box::new(fake.clone()) as Box<dyn SecretStore>);
    let options = OpenOptions::new(LOCK)
        .path(dir.0.join("data"))
        .key_file(dir.0.join("member.key"));

    Db::open_with_store(options, &store).unwrap();

    assert!(fake.0.lock().unwrap().is_empty());
}

/// The platform's own keystore: the Keychain on macOS, the Credential
/// Manager on Windows, the Secret Service on Linux. It runs where one is
/// reachable, and is skipped with the reason elsewhere, such as in CI.
#[test]
fn the_platform_keystore_keeps_the_keys_where_one_is_reachable() {
    let platform = match store::platform() {
        Ok(platform) => platform,
        Err(error) => {
            eprintln!("skipped: no platform keystore is reachable here ({error})");
            return;
        }
    };
    let dir = Dir::new("platform");
    let data = dir.0.join("data");
    let opened = Db::open(OpenOptions::new(LOCK).path(&data));
    let space = match run(opened) {
        Ok(db) => run(db.spaces().create()).unwrap().id(),
        Err(error) if error.code() == ErrorCode::KeyStoreUnavailable => {
            eprintln!("skipped: the platform keystore refused access here ({error})");
            return;
        }
        Err(error) => panic!("{error:?}"),
    };

    let reopened = run(Db::open(OpenOptions::new(LOCK).path(&data))).unwrap();
    let found = run(reopened.spaces().get(space));

    let user = data.canonicalize().unwrap().to_string_lossy().into_owned();
    platform.delete(&user).unwrap();
    found.unwrap();
}

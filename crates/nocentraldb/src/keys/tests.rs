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
use ncdb_core::membership::{Certificate, MemberKind};

const LOCK: &str = include_str!("../../tests/model/model.lock.json");

#[test]
fn a_key_record_reads_back_as_the_same_keys() {
    let keys = Keys::generate();

    let read = Keys::decode(&keys.encode()).unwrap();

    assert_eq!(read.member.key().id(), keys.member.key().id());
    assert!(read.encode() == keys.encode());
    assert_eq!(
        keys.encode().len(),
        235,
        "with the account key and the code"
    );
    assert!(!read.upgraded);
}

#[test]
fn a_record_of_another_format_or_length_is_not_read() {
    let mut record = Keys::generate().encode().to_vec();
    assert!(Keys::decode(&record[..record.len() - 1]).is_none());
    record[MAGIC.len()] = 3;
    assert!(Keys::decode(&record).is_none());
}

fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("an SDK call waited"),
    }
}

/// A record of format 1: the member's secrets and the account secret.
fn format_1(signing: u8, agreement: u8, account: u8) -> Vec<u8> {
    let mut record = MAGIC.to_vec();
    record.push(1);
    record.extend([signing; 32]);
    record.extend([agreement; 32]);
    record.extend([account; 32]);
    record
}

#[test]
fn a_record_of_format_1_gets_a_code_and_statement_1_for_its_account_key() {
    let keys = Keys::decode(&format_1(0x61, 0x62, 0x11)).unwrap();
    assert!(keys.upgraded, "it is to be written again as format 2");
    assert_eq!(
        keys.member.key().id(),
        Secrets::new([0x61; 32], [0x62; 32]).key().id()
    );
    let Account::Holds { key, statement } = &keys.account else {
        panic!("a record of format 1 holds the account key");
    };
    assert_eq!(key.to_bytes(), [0x11; 32]);
    assert_eq!(statement.number, 1);
    assert_eq!(statement.account_key, key.verifying_key().to_bytes());
    let code = keys.code.as_ref().expect("a new code");
    assert_eq!(statement.recovery_key, code.recovery_key().public());
    assert!(statement.verify());
    // Written again, it is format 2 and reads back the same.
    let again = Keys::decode(&keys.encode()).unwrap();
    assert!(!again.upgraded);
    assert!(again.encode() == keys.encode());
}

#[test]
fn a_record_waiting_to_be_paired_or_of_an_agent_reads_back() {
    let pairing = Keys::pairing();
    assert_eq!(pairing.encode().len(), 83);
    let read = Keys::decode(&pairing.encode()).unwrap();
    assert!(matches!(read.account, Account::None) && read.code.is_none());

    let device = Keys::generate();
    let Account::Holds { key, statement } = &device.account else {
        panic!("a new account holds the account key");
    };
    let member = pairing.member.key().id();
    let certificate = Certificate::sign(key, member, MemberKind::Agent);
    let agent = Keys {
        member: pairing.member.clone(),
        account: Account::Agent {
            certificate_signature: certificate.signature,
            statement: statement.clone(),
        },
        code: None,
        upgraded: false,
    };
    assert_eq!(agent.encode().len(), 283);
    let read = Keys::decode(&agent.encode()).unwrap();
    let rebuilt = read
        .account
        .certificate(member, MemberKind::Device)
        .unwrap();
    assert_eq!(
        rebuilt.kind,
        MemberKind::Agent,
        "an agent's certificate stays an agent's"
    );
    assert!(rebuilt.verify(&key.verifying_key().to_bytes()));
    assert_eq!(rebuilt.statement.as_deref(), Some(statement));
}

#[test]
fn a_record_whose_statement_does_not_verify_is_not_read() {
    let mut record = Keys::generate().encode().to_vec();
    // The statement signature's first byte, after the account secret, the
    // recovery key and the number.
    record[82 + 32 + 32 + 8] ^= 1;
    assert!(Keys::decode(&record).is_none());
}

#[test]
fn a_key_file_of_format_1_is_written_again_as_format_2_and_hands_its_code() {
    let dir = std::env::temp_dir().join(format!(
        "nocentraldb-format-1-{}-{}",
        std::process::id(),
        Zeroizing::new(random())[0]
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let key_file = dir.join("member.keys");
    std::fs::write(&key_file, format_1(0x61, 0x62, 0x11)).unwrap();
    let options = || {
        OpenOptions::new(LOCK)
            .path(dir.join("data"))
            .key_file(&key_file)
    };
    let db = now(Db::open(options())).unwrap();
    let code = now(db.account().recovery_code())
        .unwrap()
        .expect("the code of the account a format 1 record held");
    let record = std::fs::read(&key_file).unwrap();
    assert_eq!(record[MAGIC.len()], 2, "written again as format 2");
    assert_eq!(record.len(), 235, "with the code, until it is confirmed");
    let words = code.words();
    drop(db);
    let db = now(Db::open(options())).unwrap();
    let again = now(db.account().recovery_code()).unwrap().unwrap();
    assert_eq!(again.words(), words, "the same code on the next open");
    let _ = std::fs::remove_dir_all(&dir);
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

/// The key records RFC 0003 gives (docs/rfc/vectors/0003/key_record.py):
/// each reads as its state, and is written back byte for byte.
#[test]
fn the_key_record_vectors_read_as_their_state_and_write_back() {
    fn hex(text: &str) -> Vec<u8> {
        let text: String = text.split_whitespace().collect();
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }
    const DIR: &str = "../../docs/rfc/vectors/0003/";
    let read = |state: &str| -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(DIR)
            .join(format!("key-record.{state}.hex"));
        hex(&std::fs::read_to_string(path).unwrap())
    };
    let member = Secrets::new([0x61; 32], [0x62; 32]).key().id();
    for state in ["pairing", "device", "device-code", "agent"] {
        let record = read(state);
        let keys = Keys::decode(&record).unwrap_or_else(|| panic!("{state} reads"));
        assert!(!keys.upgraded, "{state}");
        assert_eq!(keys.member.key().id(), member, "{state}");
        assert!(*keys.encode() == record, "{state} is written back the same");
        let certificate = keys.account.certificate(member, MemberKind::Device);
        match state {
            "pairing" => assert!(certificate.is_none()),
            "agent" => {
                let certificate = certificate.unwrap();
                assert_eq!(certificate.kind, MemberKind::Agent);
                assert!(certificate.verify(&certificate.account_key));
            }
            _ => {
                let certificate = certificate.unwrap();
                assert!(certificate.verify(&certificate.account_key));
                assert_eq!(certificate.statement.as_ref().unwrap().number, 1);
            }
        }
        assert_eq!(keys.code.is_some(), state == "device-code", "{state}");
    }
    // Format 1 reads, with a new code for its account key, and is marked
    // to be written again.
    let old = Keys::decode(&read("format-1")).unwrap();
    assert!(old.upgraded && old.code.is_some());
    let device = Keys::decode(&read("device")).unwrap();
    let (Account::Holds { key: a, .. }, Account::Holds { key: b, .. }) =
        (&old.account, &device.account)
    else {
        panic!("both hold the account key");
    };
    assert_eq!(a.to_bytes(), b.to_bytes(), "the same account key");
}

/// A recovered account key's statement is numbered the larger of one more
/// than the highest number the device learned, none on a new device, and
/// the current time in Unix seconds (Carsten, 2026-11-12, NOC-285), and is
/// signed by the code's recovery key.
#[test]
fn a_recovered_account_key_is_numbered_by_the_time_and_signed_by_the_recovery_key() {
    let code = ncdb_core::recovery::RecoveryCode::from_bits([0xa0; 16]);
    for (now, number) in [(1_800_000_000, 1_800_000_000), (0, 1)] {
        let keys = Keys::recovered(&code, now);
        let Account::Holds { key, statement } = &keys.account else {
            panic!("a recovered database holds the new account key");
        };
        assert_eq!(statement.number, number);
        assert_eq!(statement.recovery_key, code.recovery_key().public());
        assert_eq!(statement.account_key, key.verifying_key().to_bytes());
        assert!(statement.verify());
        assert!(keys.code.is_none(), "the person holds the code");
    }
}

//! What the AWS backend does beyond the conformance suite (RFC 0007, "The
//! AWS backend"; NOC-270): where a frame is kept by its size, the orphan a
//! failed append leaves in S3 and the sweep that deletes it, appends from
//! several handles with frames on both sides of the line, the reserved
//! epoch item, and the limit of one transaction. Ignored unless an AWS
//! Emulator runs on this machine (docs/aws-emulator.md); the CI job
//! `aws-emulator` runs them.

#[path = "backend/fresh.rs"]
mod fresh;

use std::thread;

use aws_sdk_dynamodb::types::AttributeValue;
use fresh::{Aws, fresh};
use ncdb_core::change::Kind;
use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_relay::conformance::{Backend, block_on};
use ncdb_relay::store::{AppendOutcome, Effect, MemberRecord, NewEntry, RelayStore};
use ncdb_relay_aws::{EPOCH_ITEM, INLINE_LIMIT, frame_key};

fn space(n: u8) -> SpaceId {
    SpaceId([n; 16])
}

/// A change with id `n` and a frame of `size` bytes, at least 4, that tells
/// it apart.
fn entry(n: u32, size: usize) -> NewEntry {
    assert!(size >= 4, "a frame of the tests holds its change number");
    let mut change = [0xC1; 32];
    change[..4].copy_from_slice(&n.to_be_bytes());
    let mut frame = vec![n as u8; size];
    frame[..4].copy_from_slice(&n.to_be_bytes());
    NewEntry {
        change: ChangeId(change),
        accepted_at: 1_700_000_000_000 + u64::from(n),
        author: MemberId([7; 32]),
        kind: Kind::SEALED,
        frame,
    }
}

/// Whether the bucket holds an object under `key`.
fn object_exists(aws: &Aws, key: &str) -> bool {
    let s3 = aws.s3();
    let result = aws
        .runtime
        .block_on(s3.head_object().bucket(&aws.names.bucket).key(key).send());
    match result {
        Ok(_) => true,
        Err(error) if error.as_service_error().is_some_and(|e| e.is_not_found()) => false,
        Err(error) => panic!("the bucket answers: {error:?}"),
    }
}

/// The raw `log` item at `position` of `space`.
fn log_item(
    aws: &Aws,
    space: SpaceId,
    position: u64,
) -> std::collections::HashMap<String, AttributeValue> {
    let db = aws.db();
    let out = aws
        .runtime
        .block_on(
            db.get_item()
                .table_name(&aws.names.log)
                .key("space", AttributeValue::B(space.0.to_vec().into()))
                .key("position", AttributeValue::N(position.to_string()))
                .consistent_read(true)
                .send(),
        )
        .expect("the log answers");
    out.item.expect("a log item")
}

#[test]
#[ignore = "needs the AWS Emulator"]
fn a_frame_above_8_kib_goes_to_s3_and_one_of_8_kib_stays_inline() {
    let aws = fresh();
    let store = aws.connect();
    let (at_line, above) = (entry(1, INLINE_LIMIT), entry(2, INLINE_LIMIT + 1));
    let outcome = block_on(store.append(space(1), 0, vec![at_line.clone(), above.clone()], vec![]));
    assert_eq!(outcome.unwrap(), AppendOutcome::Appended { head: 2 });

    let inline = log_item(&aws, space(1), 1);
    assert!(inline.contains_key("frame"));
    assert!(!inline.contains_key("frame_key"));
    assert!(!object_exists(&aws, &frame_key(space(1), at_line.change)));

    let in_s3 = log_item(&aws, space(1), 2);
    assert!(!in_s3.contains_key("frame"));
    let key = frame_key(space(1), above.change);
    assert_eq!(
        in_s3.get("frame_key").and_then(|v| v.as_s().ok()),
        Some(&key)
    );
    assert_eq!(
        in_s3
            .get("size")
            .and_then(|v| v.as_n().ok())
            .map(String::as_str),
        Some("8193")
    );
    assert!(object_exists(&aws, &key));

    let read = block_on(store.read(space(1), 0, 10)).unwrap();
    assert_eq!(read[0].frame, at_line.frame);
    assert_eq!(read[1].frame, above.frame);
    // A bounded read counts the size of a frame in S3 without loading it.
    let bounded = block_on(store.read_bounded(space(1), 0, 10, 9_000)).unwrap();
    assert_eq!(bounded.len(), 1);
}

#[test]
#[ignore = "needs the AWS Emulator"]
fn a_failed_append_leaves_an_orphan_in_s3_which_the_sweep_deletes() {
    let aws = fresh();
    let store = aws.connect();
    let (first, lost, kept) = (
        entry(1, 10),
        entry(2, INLINE_LIMIT * 2),
        entry(3, INLINE_LIMIT * 3),
    );
    block_on(store.append(space(1), 0, vec![first], vec![])).unwrap();
    // The frame goes to S3 before the transaction, which fails on the head.
    let outcome = block_on(store.append(space(1), 0, vec![lost.clone()], vec![])).unwrap();
    assert_eq!(outcome, AppendOutcome::HeadMoved(1));
    let orphan = frame_key(space(1), lost.change);
    assert!(object_exists(&aws, &orphan), "the frame was written first");
    assert_eq!(block_on(store.find(space(1), lost.change)).unwrap(), None);
    assert_eq!(block_on(store.read(space(1), 0, 10)).unwrap().len(), 1);

    let outcome = block_on(store.append(space(1), 1, vec![kept.clone()], vec![])).unwrap();
    assert_eq!(outcome, AppendOutcome::Appended { head: 2 });
    let held = frame_key(space(1), kept.change);

    // A sweep keeps an object younger than its cutoff, orphan or not.
    assert_eq!(block_on(store.sweep_orphans(0)).unwrap(), 0);
    assert!(object_exists(&aws, &orphan));
    // A sweep with the cutoff after both objects deletes the orphan only.
    let later = now_ms() + 60_000;
    assert_eq!(block_on(store.sweep_orphans(later)).unwrap(), 1);
    assert!(!object_exists(&aws, &orphan));
    assert!(object_exists(&aws, &held));
    let read = block_on(store.read(space(1), 0, 10)).unwrap();
    assert_eq!(read[1].frame, kept.frame);
}

fn now_ms() -> u64 {
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock after 1970");
    since.as_millis() as u64
}

#[test]
#[ignore = "needs the AWS Emulator"]
fn concurrent_appends_with_frames_on_both_sides_of_the_line_stay_dense_and_exact() {
    const HANDLES: u32 = 4;
    const EACH: u32 = 8;
    let aws = fresh();
    let size = |n: u32| {
        if n.is_multiple_of(3) {
            INLINE_LIMIT + n as usize
        } else {
            n as usize * 10
        }
    };
    thread::scope(|scope| {
        for handle in 0..HANDLES {
            let aws = &aws;
            scope.spawn(move || {
                let store = aws.connect();
                for i in 0..EACH {
                    let n = handle * EACH + i + 1;
                    let mut head = block_on(store.head(space(1))).unwrap().unwrap_or(0);
                    loop {
                        let new = vec![entry(n, size(n))];
                        match block_on(store.append(space(1), head, new, vec![])).unwrap() {
                            AppendOutcome::HeadMoved(actual) => head = actual,
                            AppendOutcome::Appended { .. } => break,
                            other => panic!("{other:?}"),
                        }
                    }
                }
            });
        }
    });
    let read = block_on(aws.connect().read(space(1), 0, u32::MAX)).unwrap();
    let total = u64::from(HANDLES * EACH);
    let positions: Vec<u64> = read.iter().map(|e| e.position).collect();
    assert_eq!(positions, (1..=total).collect::<Vec<_>>());
    let mut seen: Vec<u32> = Vec::new();
    for got in &read {
        let n = u32::from_be_bytes(got.change.0[..4].try_into().unwrap());
        assert_eq!(
            got.frame,
            entry(n, size(n)).frame,
            "the frame of change {n}"
        );
        seen.push(n);
    }
    seen.sort();
    assert_eq!(seen, (1..=HANDLES * EACH).collect::<Vec<_>>());
}

#[test]
#[ignore = "needs the AWS Emulator"]
fn the_epoch_is_one_reserved_item_in_spaces() {
    let aws = fresh();
    let store = aws.connect();
    let epoch_item = || {
        let db = aws.db();
        let out = aws
            .runtime
            .block_on(
                db.get_item()
                    .table_name(&aws.names.spaces)
                    .key("space", AttributeValue::B(EPOCH_ITEM.to_vec().into()))
                    .consistent_read(true)
                    .send(),
            )
            .expect("spaces answers");
        out.item
            .and_then(|item| item.get("epoch").cloned())
            .and_then(|epoch| epoch.as_b().ok().map(|b| b.as_ref().to_vec()))
    };
    let first = block_on(store.epoch()).unwrap();
    assert_eq!(epoch_item(), Some(first.0.to_vec()));
    let second = block_on(store.new_epoch()).unwrap();
    assert_eq!(epoch_item(), Some(second.0.to_vec()));
    // The reserved item is no space: a space's head is untouched by it.
    block_on(store.append(space(1), 0, vec![entry(1, 8)], vec![])).unwrap();
    assert_eq!(block_on(store.head(space(1))).unwrap(), Some(1));
    assert_eq!(block_on(store.epoch()).unwrap(), second);
}

#[test]
#[ignore = "needs the AWS Emulator"]
fn an_append_beyond_one_transaction_fails_and_appends_nothing() {
    let aws = fresh();
    let store = aws.connect();
    // 2 × 49 + 1 = 99 items fit one transaction; one effect more does not.
    let entries: Vec<NewEntry> = (1..=49).map(|n| entry(n, 8)).collect();
    let effect = Effect::Member(MemberRecord {
        member: MemberId([1; 32]),
        current: true,
        state: vec![1],
    });
    let too_many = block_on(store.append(
        space(1),
        0,
        entries.clone(),
        vec![effect.clone(), {
            let Effect::Member(mut other) = effect.clone();
            other.member = MemberId([2; 32]);
            Effect::Member(other)
        }],
    ));
    assert!(too_many.is_err(), "{too_many:?}");
    assert_eq!(block_on(store.head(space(1))).unwrap(), None);
    let fits = block_on(store.append(space(1), 0, entries, vec![effect])).unwrap();
    assert_eq!(fits, AppendOutcome::Appended { head: 49 });
}

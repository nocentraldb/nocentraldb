//! The suite's rules for admissions (RFC 0015, "How the relay takes it";
//! NOC-469): `admit` records an identity's until, spaces and admission key,
//! a later one replaces them and keeps the count of spaces created, and an
//! append that records an admitted space holds only while the count is
//! below spaces, even when two appends race.

use std::thread;

use super::{Backend, block_on, entry, space};
use crate::store::{AdmissionRecord, Admitted, AppendOutcome, Effect, RelayStore};

const KEY: [u8; 32] = [0x4b; 32];

fn admitted(until: u64, spaces: u64) -> Admitted {
    Admitted {
        until,
        spaces,
        key: KEY,
    }
}

pub fn an_admission_is_recorded_and_replaced_with_its_count_kept<B: Backend>(backend: &B) {
    let store = backend.connect();
    let (identity, other) = ([0x41; 32], [0x42; 32]);
    assert_eq!(block_on(store.admission(identity)).unwrap(), None);

    block_on(store.admit(identity, admitted(100, 2))).unwrap();
    let effect = Effect::AdmittedSpace(identity);
    let appended = block_on(store.append(space(1), 0, vec![entry(1)], vec![effect]));
    assert_eq!(appended.unwrap(), AppendOutcome::Appended { head: 1 });

    let fresh = backend.connect();
    assert_eq!(
        block_on(fresh.admission(identity)).unwrap(),
        Some(AdmissionRecord {
            until: 100,
            spaces: 2,
            key: KEY,
            created: 1,
        })
    );
    // A later admission replaces until, spaces and the key; the count stays.
    let later = Admitted {
        until: 200,
        spaces: 5,
        key: [0x4c; 32],
    };
    block_on(store.admit(identity, later)).unwrap();
    assert_eq!(
        block_on(fresh.admission(identity)).unwrap(),
        Some(AdmissionRecord {
            until: 200,
            spaces: 5,
            key: [0x4c; 32],
            created: 1,
        })
    );
    // Another identity has its own record.
    assert_eq!(block_on(fresh.admission(other)).unwrap(), None);
}

pub fn an_admitted_space_holds_only_below_spaces<B: Backend>(backend: &B) {
    let store = backend.connect();
    let (identity, unknown) = ([0x43; 32], [0x44; 32]);
    block_on(store.admit(identity, admitted(100, 1))).unwrap();
    let first = block_on(store.append(
        space(1),
        0,
        vec![entry(1)],
        vec![Effect::AdmittedSpace(identity)],
    ));
    assert_eq!(first.unwrap(), AppendOutcome::Appended { head: 1 });

    let second = block_on(store.append(
        space(2),
        0,
        vec![entry(2)],
        vec![Effect::AdmittedSpace(identity)],
    ));
    assert_eq!(second.unwrap(), AppendOutcome::AdmissionSpent);
    // Nothing of the refused append is applied, through any handle.
    let fresh = backend.connect();
    assert_eq!(block_on(fresh.head(space(2))).unwrap(), None);
    assert_eq!(
        block_on(fresh.find(space(2), entry(2).change)).unwrap(),
        None
    );
    assert_eq!(
        block_on(fresh.admission(identity))
            .unwrap()
            .unwrap()
            .created,
        1
    );

    // An identity without an admission records no admitted space.
    let none = block_on(store.append(
        space(3),
        0,
        vec![entry(3)],
        vec![Effect::AdmittedSpace(unknown)],
    ));
    assert_eq!(none.unwrap(), AppendOutcome::AdmissionSpent);
    assert_eq!(block_on(fresh.head(space(3))).unwrap(), None);
}

pub fn concurrent_admitted_spaces_do_not_exceed_spaces<B: Backend>(backend: &B) {
    let identity = [0x45; 32];
    block_on(backend.connect().admit(identity, admitted(100, 1))).unwrap();
    let outcomes: Vec<AppendOutcome> = thread::scope(|scope| {
        let racers: Vec<_> = (1..=4u8)
            .map(|at| {
                let store = backend.connect();
                scope.spawn(move || {
                    let effect = Effect::AdmittedSpace(identity);
                    block_on(store.append(space(at), 0, vec![entry(u32::from(at))], vec![effect]))
                        .unwrap()
                })
            })
            .collect();
        racers.into_iter().map(|r| r.join().unwrap()).collect()
    });
    let held = outcomes
        .iter()
        .filter(|o| matches!(o, AppendOutcome::Appended { .. }))
        .count();
    assert_eq!(held, 1, "{outcomes:?}");
    assert!(
        outcomes.iter().all(|o| matches!(
            o,
            AppendOutcome::Appended { .. } | AppendOutcome::AdmissionSpent
        )),
        "{outcomes:?}"
    );
    let record = block_on(backend.connect().admission(identity))
        .unwrap()
        .unwrap();
    assert_eq!(record.created, 1);
}

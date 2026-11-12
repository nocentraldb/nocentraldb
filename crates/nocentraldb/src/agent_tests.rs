//! `agent()` marks the member as an agent in the certificate its account
//! signs (RFC 0009, "Accounts and devices"; RFC 0003, "Member
//! certificate"), so a space adds
//! it as a writer when no role is given (RFC 0008, "Roles and rights"); without it, the
//! member is a device.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets (NOC-175).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::log::Position;
use ncdb_core::membership::{ControlBody, Entry, MemberKind};

use crate::{Db, OpenOptions};

const LOCK: &str = include_str!("../tests/model/model.lock.json");

/// The kind the genesis of a new space certifies its creator as.
fn creator_kind(options: OpenOptions) -> MemberKind {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    let db = match std::pin::pin!(Db::open(options)).poll(&mut cx) {
        std::task::Poll::Ready(db) => db.unwrap(),
        std::task::Poll::Pending => panic!("the SDK never waits on the device"),
    };
    let space = match std::pin::pin!(db.spaces().create()).poll(&mut cx) {
        std::task::Poll::Ready(space) => space.unwrap(),
        std::task::Poll::Pending => panic!("the SDK never waits on the device"),
    };
    let inner = db.lock();
    let log = inner.view.log(space.id().0).unwrap();
    let (_, genesis) = log.after(Position::START).next().unwrap();
    let body = ControlBody::decode(&genesis.header().payload).unwrap();
    let Entry::Genesis(genesis) = body.entry else {
        panic!("the first change of a space is its genesis");
    };
    genesis.certificate.kind
}

#[test]
fn an_agent_is_certified_as_an_agent() {
    let kind = creator_kind(OpenOptions::new(LOCK).in_memory().agent());

    assert_eq!(kind, MemberKind::Agent);
}

#[test]
fn without_agent_the_member_is_certified_as_a_device() {
    let kind = creator_kind(OpenOptions::new(LOCK).in_memory());

    assert_eq!(kind, MemberKind::Device);
}

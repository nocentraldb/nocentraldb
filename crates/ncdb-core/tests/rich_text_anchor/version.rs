//! The read version and its cache (RFC 0013, "An agent: anchored edits",
//! "The version"): the document's state at the read, held in a
//! device-local cache with a time limit. An edit that names a version the
//! cache no longer holds is refused, and the agent reads again.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::rich_text::anchor::{Error, Version, Versions};

use crate::doc::{p, t};
use crate::support::{Field, LIMIT, NOW, key, lease, replace};

fn field() -> Field {
    Field::new(vec![p(vec![t("We ship on Friday.")])])
}

#[test]
fn a_read_gives_the_markdown_and_a_version() {
    let field = field();
    let mut versions = Versions::new(LIMIT);
    let one = versions.read(key(), &field.state, NOW);
    let two = versions.read(key(), &field.state, NOW);
    assert_eq!(one.markdown, "We ship on Friday.");
    assert_ne!(one.version, two.version);
    let printed = format!("{one:?}");
    assert!(!printed.contains("Friday"), "{printed}");
}

#[test]
fn a_version_is_held_until_its_time_limit_passes() {
    let mut field = field();
    let mut versions = Versions::new(LIMIT);
    let reading = versions.read(key(), &field.state, NOW);
    let mut lease = lease();
    let written = versions
        .edit(
            &reading.version,
            &mut lease,
            &[replace("Friday", "Monday")],
            NOW + LIMIT - 1,
        )
        .unwrap();
    field.agent(&written);
    assert_eq!(
        versions.edit(
            &reading.version,
            &mut lease,
            &[replace("Monday", "Tuesday")],
            NOW + LIMIT
        ),
        Err(Error::VersionUnknown)
    );
}

#[test]
fn a_version_the_cache_never_held_is_unknown() {
    let mut versions = Versions::new(LIMIT);
    assert_eq!(
        versions.edit(&Version([1; 16]), &mut lease(), &[replace("a", "b")], NOW),
        Err(Error::VersionUnknown)
    );
}

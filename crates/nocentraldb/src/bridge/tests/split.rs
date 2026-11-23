//! A write above the relay's change limit through the bridge, as the
//! TypeScript SDK sends it (NOC-421; NOC-431, B′): many objects are split
//! and answered as one write; one operation above the limit is refused
//! with the error `tooLarge`.

use super::*;

const LIMIT: usize = 16 * 1024 * 1024;

fn create(title: &str) -> Value {
    map(&[
        ("op", text("create")),
        ("collection", text("tasks")),
        ("values", map(&[("title", text(title))])),
    ])
}

#[test]
fn a_write_of_many_objects_above_the_limit_answers_every_id() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let title = "x".repeat(256 * 1024);
    let ops = (0..72).map(|_| create(&title)).collect();

    let written = ok(
        &mut bridge,
        "write",
        map(&[("space", space), ("ops", Value::Array(ops))]),
    );

    match field(&written, "ids") {
        Some(Value::Array(ids)) => assert_eq!(ids.len(), 72),
        other => panic!("no ids: {other:?}"),
    }
}

#[test]
fn one_operation_above_the_limit_is_too_large() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let ops = vec![create(&"x".repeat(LIMIT + 1))];

    assert_eq!(
        code(
            &mut bridge,
            "write",
            map(&[("space", space), ("ops", Value::Array(ops))])
        ),
        "tooLarge"
    );
}

//! A day and a point in time (RFC 0005, "Days and points in time"; Carsten,
//! 2026-11-21, NOC-434): `Date` and `Timestamp`, the SDK's own types,
//! written with `Value::from`, read with `get::<Date>` and
//! `get::<Timestamp>`, cleared with `Value::clear()`. Each refuses a value
//! outside the years 1 to 9999, a field of another type refuses them, and a
//! declared index sorts points in time in time order.

mod support;

use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nocentraldb::{Date, ErrorCode, Timestamp, Value};
use support::{now, open};

#[test]
fn a_date_is_a_day_of_the_years_1_to_9999() {
    let day = Date::new(2026, 11, 21).unwrap();
    assert_eq!((day.year(), day.month(), day.day()), (2026, 11, 21));
    assert_eq!(day.to_string(), "2026-11-21");
    assert_eq!(Date::from_str("2026-11-21").unwrap(), day);
    assert_eq!(
        Date::from_str("1969-12-31").unwrap().to_string(),
        "1969-12-31"
    );
    assert_eq!(Date::new(1, 1, 1).unwrap().to_string(), "0001-01-01");
    assert_eq!(Date::new(9999, 12, 31).unwrap().to_string(), "9999-12-31");
    assert_eq!(Date::new(2024, 2, 29).unwrap().to_string(), "2024-02-29");
    for (y, m, d) in [
        (2026, 2, 29),
        (2026, 2, 30),
        (2026, 13, 1),
        (2026, 4, 31),
        (0, 12, 31),
        (10000, 1, 1),
        (2026, 0, 1),
        (2026, 1, 0),
    ] {
        let refused = Date::new(y, m, d).unwrap_err();
        assert_eq!(refused.code(), ErrorCode::InvalidValue, "{y}-{m}-{d}");
    }
    for text in [
        "2026-1-21",
        "21.11.2026",
        "2026-11-21T00:00",
        "+2026-11-21",
        "2026-02-30",
        "",
    ] {
        assert_eq!(
            Date::from_str(text).unwrap_err().code(),
            ErrorCode::InvalidValue,
            "{text:?}"
        );
    }
}

#[test]
fn a_timestamp_is_a_millisecond_of_the_years_1_to_9999_in_utc() {
    let point = Timestamp::from_millis(1_795_288_124_636).unwrap();
    assert_eq!(point.as_millis(), 1_795_288_124_636);
    assert_eq!(point.to_string(), "2026-11-21T19:08:44.636Z");
    assert_eq!(
        Timestamp::from_millis(-1).unwrap().to_string(),
        "1969-12-31T23:59:59.999Z"
    );
    let first = Timestamp::from_millis(-62_135_596_800_000).unwrap();
    assert_eq!(first.to_string(), "0001-01-01T00:00:00.000Z");
    let last = Timestamp::from_millis(253_402_300_799_999).unwrap();
    assert_eq!(last.to_string(), "9999-12-31T23:59:59.999Z");
    for millis in [-62_135_596_800_001, 253_402_300_800_000, i64::MIN, i64::MAX] {
        assert_eq!(
            Timestamp::from_millis(millis).unwrap_err().code(),
            ErrorCode::InvalidValue,
            "{millis}"
        );
    }
}

#[test]
fn a_system_time_rounds_down_to_the_millisecond_toward_the_past() {
    let after = UNIX_EPOCH + Duration::from_nanos(1_795_288_124_636_999_999);
    assert_eq!(
        Timestamp::try_from(after).unwrap().as_millis(),
        1_795_288_124_636
    );
    let before = UNIX_EPOCH - Duration::from_nanos(1);
    assert_eq!(Timestamp::try_from(before).unwrap().as_millis(), -1);
    let point = Timestamp::from_millis(1_795_288_124_636).unwrap();
    assert_eq!(
        SystemTime::try_from(point).unwrap(),
        UNIX_EPOCH + Duration::from_millis(1_795_288_124_636)
    );
    let far = UNIX_EPOCH + Duration::from_secs(300_000_000_000);
    assert_eq!(
        Timestamp::try_from(far).unwrap_err().code(),
        ErrorCode::InvalidValue
    );
}

#[test]
fn a_day_and_a_point_in_time_are_written_read_back_and_cleared() {
    let db = open();
    let space = now(db.spaces().create()).unwrap();
    let day = Date::new(2026, 11, 21).unwrap();
    let point = Timestamp::from_millis(1_795_288_124_636).unwrap();
    let task = now(space.create(
        "tasks",
        [("doneOn", Value::from(day)), ("dueAt", Value::from(point))],
    ))
    .unwrap()
    .ids[0];

    let read = now(space.get("tasks", task)).unwrap().unwrap();
    assert_eq!(read.get::<Date>("doneOn").unwrap(), Some(day));
    assert_eq!(read.get::<Timestamp>("dueAt").unwrap(), Some(point));
    // A day is no integer, and an integer field holds no day.
    assert_eq!(
        read.get::<i64>("doneOn").unwrap_err().code(),
        ErrorCode::InvalidValue
    );

    now(space.update(
        "tasks",
        task,
        [("doneOn", Value::from(Date::new(1969, 12, 31).unwrap()))],
    ))
    .unwrap();
    now(space.update("tasks", task, [("dueAt", Value::clear())])).unwrap();
    let read = now(space.get("tasks", task)).unwrap().unwrap();
    assert_eq!(
        read.get::<Date>("doneOn").unwrap(),
        Some(Date::new(1969, 12, 31).unwrap())
    );
    assert_eq!(read.get::<Timestamp>("dueAt").unwrap(), None);
}

#[test]
fn a_field_takes_only_its_own_kind_of_date() {
    let db = open();
    let space = now(db.spaces().create()).unwrap();
    let day = Value::from(Date::new(2026, 11, 21).unwrap());
    let point = Value::from(Timestamp::from_millis(0).unwrap());
    for (field, value) in [
        ("doneOn", Value::from(20_778)),
        ("doneOn", Value::from("2026-11-21")),
        ("doneOn", point.clone()),
        ("dueAt", Value::from(0)),
        ("dueAt", day.clone()),
        ("priority", day),
        ("title", point),
    ] {
        let refused = now(space.create("tasks", [(field, value)])).unwrap_err();
        assert_eq!(refused.code(), ErrorCode::InvalidValue, "{field}");
    }
}

#[test]
fn an_index_sorts_points_in_time_in_time_order() {
    let db = open();
    let space = now(db.spaces().create()).unwrap();
    for (title, millis) in [("now", 1_795_288_124_636), ("1969", -1), ("1970", 0)] {
        let point = Timestamp::from_millis(millis).unwrap();
        now(space.create(
            "tasks",
            [("title", Value::from(title)), ("dueAt", Value::from(point))],
        ))
        .unwrap();
    }
    let query = db.index("tasks", "byDueAt").unwrap().take(10);
    let page = now(db.find(&query)).unwrap();
    let titles: Vec<&str> = page
        .items
        .iter()
        .map(|item| item.get::<&str>("title").unwrap().unwrap())
        .collect();
    // Descending: the latest first.
    assert_eq!(titles, ["now", "1970", "1969"]);
}

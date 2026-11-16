//! Ad-hoc spaces for sub-agents (NOC-318; RFC 0003, "Sub-agents get an
//! ad-hoc space"; RFC 0009, "Sub-agents and ad-hoc spaces"; Carsten,
//! 2026-11-15, NOC-320 Q4 to Q8 and Q10): `Spaces::create_ad_hoc` creates a
//! space with a fresh key and copies the given objects into it as new
//! objects, with fresh ids. A copied link points to the copy of its target,
//! or is cleared; personal fields are not copied; the parent keeps which
//! object each copy came from, on its own database only.

mod support;

use nocentraldb::{
    AdHoc, Data, Db, ErrorCode, NoticeCode, Object, ObjectId, OpenOptions, Space, SpaceId, Value,
};
use support::{LOCK, TempDir, now};

/// The tests' lock with `related`, many links to tasks, and `votes`, a
/// field of the test plugin `test/counter`, on tasks. The lock's hash is
/// not checked on read.
fn lock() -> String {
    let lock = LOCK.replacen(
        r#""estimate": { "rule": 1, "since": 1, "type": "ncdb/float" },"#,
        r#""estimate": { "rule": 1, "since": 1, "type": "ncdb/float" },
        "related": { "rule": 1, "since": 1, "targets": ["tasks"], "type": "ncdb/links" },
        "votes": { "rule": 1, "since": 1, "type": "test/counter" },"#,
        1,
    );
    let lock = lock.replacen(
        r#""plugins": {}"#,
        r#""plugins": { "test": { "crate": "ncdb-test-counter" } }"#,
        1,
    );
    assert!(lock.contains("ncdb/links") && lock.contains("ncdb-test-counter"));
    lock
}

fn open(lock: &str) -> Db {
    now(Db::open(
        OpenOptions::new(lock)
            .in_memory()
            .plugin(ncdb_test_counter::plugin()),
    ))
    .unwrap()
}

fn read(space: &Space, collection: &str, id: ObjectId) -> Object {
    now(space.get(collection, id)).unwrap().unwrap()
}

fn ad_hoc(copy: Vec<Object>) -> AdHoc {
    AdHoc {
        copy,
        ..AdHoc::default()
    }
}

/// A project and a task of it, with every basic field and a tag set.
fn project_and_task(space: &Space) -> (ObjectId, ObjectId) {
    let written = now(space.write(|tx| {
        let project = tx.create("projects", [("name", Value::from("Launch"))])?;
        tx.create(
            "tasks",
            [
                ("title", Value::from("Write the notes")),
                ("status", Value::from("open")),
                ("priority", Value::from(2)),
                ("estimate", Value::from(1.5)),
                ("urgent", Value::from(true)),
                ("tags", Value::add(["docs", "release"])),
                ("project", Value::from(project)),
            ],
        )?;
        Ok(())
    }))
    .unwrap();
    (written.ids[0], written.ids[1])
}

#[test]
fn the_copies_are_new_objects_of_a_new_space_with_the_same_values() {
    let db = open(&lock());
    let project = now(db.spaces().create()).unwrap();
    let (p, t) = project_and_task(&project);

    let created = now(db.spaces().create_ad_hoc(ad_hoc(vec![
        read(&project, "projects", p),
        read(&project, "tasks", t),
    ])))
    .unwrap();

    assert_eq!(created.notice.code, NoticeCode::AdHocSpaceCreated);
    assert_ne!(created.space.id(), project.id());
    assert_eq!(created.cleared_links, 0);
    let [(source_p, copy_p), (source_t, copy_t)] = created.copies[..] else {
        panic!("two copies, in the order of `copy`: {:?}", created.copies);
    };
    assert_eq!((source_p, source_t), (p, t));
    assert!(copy_p != p && copy_t != t, "every copy gets a fresh id");

    let copy = read(&created.space, "tasks", copy_t);
    assert_eq!(copy.space(), created.space.id());
    assert_eq!(copy.get::<&str>("title").unwrap(), Some("Write the notes"));
    assert_eq!(copy.get::<&str>("status").unwrap(), Some("open"));
    assert_eq!(copy.get::<i64>("priority").unwrap(), Some(2));
    assert_eq!(copy.get::<f64>("estimate").unwrap(), Some(1.5));
    assert_eq!(copy.get::<bool>("urgent").unwrap(), Some(true));
    let tags: Vec<&str> = copy
        .get::<&[Value]>("tags")
        .unwrap()
        .unwrap()
        .iter()
        .map(|tag| tag.try_into().unwrap())
        .collect();
    assert_eq!(tags, ["docs", "release"]);
    // The link points to the copy of its target, never to the source.
    assert_eq!(copy.get::<ObjectId>("project").unwrap(), Some(copy_p));
    let name = read(&created.space, "projects", copy_p);
    assert_eq!(name.get::<&str>("name").unwrap(), Some("Launch"));

    // The sources stay as they were.
    let source = read(&project, "tasks", t);
    assert_eq!(source.get::<ObjectId>("project").unwrap(), Some(p));
}

#[test]
fn the_parent_finds_each_copys_source_and_each_sources_copy() {
    let db = open(&lock());
    let project = now(db.spaces().create()).unwrap();
    let (p, t) = project_and_task(&project);
    let created = now(db
        .spaces()
        .create_ad_hoc(ad_hoc(vec![read(&project, "tasks", t)])))
    .unwrap();
    let copy = created.copies[0].1;

    let space = &created.space;
    assert_eq!(now(space.source_of(copy)).unwrap(), Some((project.id(), t)));
    assert_eq!(now(space.copy_of(project.id(), t)).unwrap(), Some(copy));
    assert_eq!(now(space.copy_of(project.id(), p)).unwrap(), None);
    assert_eq!(now(space.source_of(t)).unwrap(), None);
    // The source space holds no copies.
    assert_eq!(now(project.source_of(t)).unwrap(), None);
}

#[test]
fn a_link_whose_target_is_not_copied_is_cleared_and_counted() {
    let db = open(&lock());
    let project = now(db.spaces().create()).unwrap();
    let (_, t) = project_and_task(&project);

    let created = now(db
        .spaces()
        .create_ad_hoc(ad_hoc(vec![read(&project, "tasks", t)])))
    .unwrap();

    assert_eq!(created.cleared_links, 1);
    let copy = read(&created.space, "tasks", created.copies[0].1);
    assert_eq!(copy.get::<ObjectId>("project").unwrap(), None);
    assert_eq!(copy.get::<&str>("title").unwrap(), Some("Write the notes"));
}

#[test]
fn many_links_keep_the_copied_targets_and_drop_the_others() {
    let db = open(&lock());
    let project = now(db.spaces().create()).unwrap();
    let written = now(project.write(|tx| {
        let a = tx.create("tasks", [("title", Value::from("a"))])?;
        let b = tx.create("tasks", [("title", Value::from("b"))])?;
        tx.create(
            "tasks",
            [("title", Value::from("c")), ("related", Value::add([a, b]))],
        )?;
        Ok(())
    }))
    .unwrap();
    let [a, _, c] = written.ids[..] else {
        panic!("three tasks");
    };

    let created = now(db.spaces().create_ad_hoc(ad_hoc(vec![
        read(&project, "tasks", c),
        read(&project, "tasks", a),
    ])))
    .unwrap();

    assert_eq!(created.cleared_links, 1);
    let copy_c = now(created.space.copy_of(project.id(), c))
        .unwrap()
        .unwrap();
    let copy_a = now(created.space.copy_of(project.id(), a))
        .unwrap()
        .unwrap();
    let related: Vec<ObjectId> = read(&created.space, "tasks", copy_c)
        .get::<&[Value]>("related")
        .unwrap()
        .unwrap()
        .iter()
        .map(|id| id.try_into().unwrap())
        .collect();
    assert_eq!(related, [copy_a]);
}

#[test]
fn copies_come_from_several_spaces() {
    let db = open(&lock());
    let first = now(db.spaces().create()).unwrap();
    let second = now(db.spaces().create()).unwrap();
    let (_, t1) = project_and_task(&first);
    let (_, t2) = project_and_task(&second);

    let created = now(db.spaces().create_ad_hoc(ad_hoc(vec![
        read(&first, "tasks", t1),
        read(&second, "tasks", t2),
    ])))
    .unwrap();

    let space = &created.space;
    let [(_, c1), (_, c2)] = created.copies[..] else {
        panic!("two copies");
    };
    assert_eq!(now(space.source_of(c1)).unwrap(), Some((first.id(), t1)));
    assert_eq!(now(space.source_of(c2)).unwrap(), Some((second.id(), t2)));
}

/// The ids of the spaces `db` holds.
fn spaces(db: &Db) -> Vec<SpaceId> {
    now(db.spaces().list())
        .unwrap()
        .into_iter()
        .map(|space| space.id)
        .collect()
}

#[test]
fn a_deleted_source_or_one_named_twice_is_refused_and_creates_nothing() {
    let db = open(&lock());
    let project = now(db.spaces().create()).unwrap();
    let (p, t) = project_and_task(&project);
    let task = read(&project, "tasks", t);
    now(project.delete("projects", p)).unwrap();
    let before = spaces(&db);

    let deleted = read(&project, "projects", p);
    let refused = now(db
        .spaces()
        .create_ad_hoc(ad_hoc(vec![task.clone(), deleted])))
    .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::InvalidValue);

    let refused = now(db.spaces().create_ad_hoc(ad_hoc(vec![task.clone(), task]))).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::InvalidValue);

    assert_eq!(spaces(&db), before);
}

// The message names the space, then the object, as its text says.
#[test]
fn a_source_this_database_does_not_hold_is_not_found_and_named() {
    let other = open(&lock());
    let project = now(other.spaces().create()).unwrap();
    let (_, t) = project_and_task(&project);
    let task = read(&project, "tasks", t);

    let db = open(&lock());
    let refused = now(db.spaces().create_ad_hoc(ad_hoc(vec![task]))).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotFound);
    assert_eq!(
        refused.to_string(),
        format!("the space {} holds no object {t} to copy", project.id()),
    );
}

#[test]
fn personal_and_local_ad_hoc_spaces_come_later() {
    let db = open(&lock());
    let project = now(db.spaces().create()).unwrap();
    let (_, t) = project_and_task(&project);
    let before = spaces(&db);
    for (options, why) in [
        (
            AdHoc {
                personal: true,
                ..ad_hoc(vec![read(&project, "tasks", t)])
            },
            "personal",
        ),
        (
            AdHoc {
                local: true,
                ..ad_hoc(vec![read(&project, "tasks", t)])
            },
            "local",
        ),
    ] {
        let refused = now(db.spaces().create_ad_hoc(options)).unwrap_err();
        assert_eq!(refused.code(), ErrorCode::NotAllowed, "{why}");
    }
    assert_eq!(spaces(&db), before);
}

#[test]
fn personal_fields_are_not_copied() {
    // `tasks.title` is personal to the task's project.
    let lock = lock().replacen(
        r#""title": { "rule": 1, "since": 1, "type": "ncdb/string" }"#,
        r#""title": { "personal": { "person": "project" }, "rule": 1, "since": 1, "type": "ncdb/string" }"#,
        1,
    );
    assert!(lock.contains(r#""person": "project""#));
    let db = open(&lock);
    let project = now(db.spaces().create()).unwrap();
    let (p, t) = project_and_task(&project);
    assert_eq!(
        read(&project, "tasks", t).get::<&str>("title").unwrap(),
        Some("Write the notes")
    );

    let created = now(db.spaces().create_ad_hoc(ad_hoc(vec![
        read(&project, "projects", p),
        read(&project, "tasks", t),
    ])))
    .unwrap();

    let copy = read(&created.space, "tasks", created.copies[1].1);
    assert_eq!(copy.get::<&str>("title").unwrap(), None);
    assert_eq!(copy.get::<&str>("status").unwrap(), Some("open"));
}

#[test]
fn a_plugin_field_is_not_copied_and_is_counted() {
    let db = open(&lock());
    let project = now(db.spaces().create()).unwrap();
    let (_, t) = project_and_task(&project);
    now(project.update("tasks", t, [("votes", Value::from(Data::Integer(3)))])).unwrap();

    let (_, unvoted) = project_and_task(&project);

    let created = now(db.spaces().create_ad_hoc(ad_hoc(vec![
        read(&project, "tasks", t),
        read(&project, "tasks", unvoted),
    ])))
    .unwrap();

    // Only the field that was written counts.
    assert_eq!(created.plugin_fields_not_copied, 1);
    let copy = read(&created.space, "tasks", created.copies[0].1);
    assert_ne!(copy.get::<Data>("votes").unwrap(), Some(Data::Integer(3)));
}

#[test]
fn the_parent_keeps_its_copies_across_a_reopen() {
    let dir = TempDir::new("ad-hoc-reopen");
    let lock = lock();
    let at = || {
        OpenOptions::new(&lock)
            .path(dir.join("data"))
            .key_file(dir.join("member.key"))
            .plugin(ncdb_test_counter::plugin())
    };
    let (source_space, t, ad_hoc_space, copy) = {
        let db = now(Db::open(at())).unwrap();
        let project = now(db.spaces().create()).unwrap();
        let (_, t) = project_and_task(&project);
        let created = now(db
            .spaces()
            .create_ad_hoc(ad_hoc(vec![read(&project, "tasks", t)])))
        .unwrap();
        (project.id(), t, created.space.id(), created.copies[0].1)
    };
    let db = now(Db::open(at())).unwrap();
    let space = now(db.spaces().get(ad_hoc_space)).unwrap();
    assert_eq!(now(space.source_of(copy)).unwrap(), Some((source_space, t)));
}

#[test]
fn a_sub_agent_creates_no_ad_hoc_space() {
    let sub_agent = now(Db::open(OpenOptions::new(LOCK).in_memory().sub_agent())).unwrap();
    let refused = now(sub_agent.spaces().create_ad_hoc(AdHoc::default())).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotAllowed);
}

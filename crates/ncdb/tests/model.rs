//! `ncdb model build` and `ncdb model check` (RFC 0005, "The structure of
//! model.lock.json" and "An example of each"), run as a developer runs
//! them: on a TypeScript schema, through Node.js, in a project directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use ncdb_core::model_lock::{Draft, ModelId, ModelLock, Scope, build};

/// RFC 0005's lock at model version 2 (keys in canonical order).
const EXAMPLE: &str = include_str!("../../ncdb-core/tests/model_lock/rfc-0005-v2.lock.json");

const ID: ModelId = ModelId([
    0x6f, 0x1c, 0x0e, 0x2a, 0x9b, 0x7d, 0x4c, 0x3e, 0x8a, 0x5f, 0x0b, 0x1d, 0x2c, 0x3e, 0x4f, 0x5a,
]);

/// The task board's schema at model version 1 (RFC 0005), without the
/// rich-text body.
const SCHEMA_V1: &str = r#"
export const model = nc.model({
  tasks: nc.collection(
    {
      title: nc.string().describe("One line; what has to be done."),
      status: nc.oneOf(["open", "in_progress", "done"]),
      priority: nc.int(),
      dependsOn: nc.links("tasks"),
    },
    {
      computed: { blocked: nc.reach("dependsOn", { status: ["open", "in_progress"] }) },
      views: { ready: nc.view("tasks", { where: { blocked: false }, sort: ["priority"] }) },
      indexes: { byStatusPriority: nc.index(["status", "priority"], { equal: 1 }) },
    },
  ),
});
"#;

/// The task board's schema at model version 2 (RFC 0005), without the
/// rich-text body.
const SCHEMA_V2: &str = r#"
export const model = nc.model({
  people: nc.collection({
    name: nc.string().personal({ person: "self" }),
    email: nc.string().personal({ person: "self" }),
  }),
  tasks: nc.collection(
    {
      title: nc.string().describe("One line; what has to be done."),
      status: nc.oneOf(["open", "in_progress", "done"]),
      priority: nc.int(),
      estimate: nc.float(),
      archived: nc.boolean(),
      tags: nc.set(nc.string()),
      assignee: nc.link("people"),
      notesAboutAssignee: nc.string().personal({ person: "assignee" }),
      dependsOn: nc.links("tasks"),
    },
    {
      computed: { blocked: nc.reach("dependsOn", { status: ["open", "in_progress"] }) },
      views: { ready: nc.view("tasks", { where: { blocked: false }, sort: ["priority"] }) },
      indexes: {
        byStatusPriority: nc.index(["status", "priority"], { equal: 1 }),
        byPriorityDesc: nc.index([{ field: "priority", direction: "desc" }]),
      },
    },
  ),
});

export type Task = { title?: string };
"#;

/// A fresh project directory.
fn project() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ncdb-model-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes a schema module that imports the SDK of this repository.
fn write_schema(path: &Path, body: &str) {
    let sdk = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/sdk/src/index.ts");
    let sdk = fs::canonicalize(sdk).unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!("import {{ nc }} from {:?};\n{body}", sdk.to_str().unwrap()),
    )
    .unwrap();
}

/// The lock of version 1 with RFC 0005's model.id.
fn lock_v1() -> String {
    let draft = r#"{ "plugins": {}, "collections": { "tasks": {
        "fields": {
          "title": { "description": "One line; what has to be done.", "rule": 1, "type": "ncdb/string" },
          "status": { "options": ["open", "in_progress", "done"], "rule": 1, "type": "ncdb/choice" },
          "priority": { "rule": 1, "type": "ncdb/int" },
          "dependsOn": { "rule": 1, "targets": ["tasks"], "type": "ncdb/links" } },
        "computed": ["blocked"], "views": ["ready"],
        "indexes": { "byStatusPriority": { "equal": 1, "fields": [{ "direction": "asc", "field": "status" }, { "direction": "asc", "field": "priority" }] } } } } }"#;
    let draft = Draft::from_json(draft).unwrap();
    build(&draft, None, || ID).unwrap().lock.to_canonical_json()
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

fn ncdb(dir: &Path, args: &[&str]) -> Run {
    ncdb_with(dir, args, |_| {})
}

fn ncdb_with(dir: &Path, args: &[&str], setup: impl FnOnce(&mut Command)) -> Run {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ncdb"));
    command.args(args).current_dir(dir);
    setup(&mut command);
    let output = command.output().unwrap();
    Run {
        code: output.status.code().unwrap(),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

/// Writes a fake `node` into `dir/bin` that prints `version`, and returns `dir/bin`.
///
/// A child `sh` writes the script, never this process: a file this process
/// held open for writing would leak into every child another test thread
/// forks meanwhile, and running the script while such a child lives fails
/// with ETXTBSY (NOC-94). Renaming a finished file into place would not help,
/// since the leaked descriptor points at the same file.
#[cfg(unix)]
fn fake_node(dir: &Path, version: &str) -> PathBuf {
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let status = Command::new("/bin/sh")
        .args([
            "-c",
            r#"printf '#!/bin/sh\necho %s\n' "$1" > node && chmod 755 node"#,
            "sh",
        ])
        .arg(version)
        .current_dir(&bin)
        .status()
        .unwrap();
    assert!(status.success());
    bin
}

fn read(path: PathBuf) -> String {
    fs::read_to_string(path).unwrap()
}

#[test]
fn the_first_build_writes_version_1_with_a_new_model_id() {
    let dir = project();
    write_schema(&dir.join("model.ts"), SCHEMA_V1);
    let run = ncdb(&dir, &["model", "build"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let lock = ModelLock::from_json(&read(dir.join("model.lock.json"))).unwrap();
    assert_eq!(lock.model.version, 1);
    assert_eq!(
        run.stdout,
        format!(
            "model.lock.json: model version 1, a new model ({})\n  + collection tasks (4 fields)\n",
            lock.model.id
        )
    );
    assert_eq!(read(dir.join("model.lock.json")), lock.to_canonical_json());
}

#[test]
fn building_version_2_writes_the_rfc_example_and_lists_what_it_added() {
    let dir = project();
    write_schema(&dir.join("model.ts"), SCHEMA_V2);
    fs::write(dir.join("model.lock.json"), lock_v1()).unwrap();
    let run = ncdb(&dir, &["model", "build"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(read(dir.join("model.lock.json")), EXAMPLE);
    assert_eq!(
        run.stdout,
        "model.lock.json: model version 1 -> 2
  + collection people (2 personal fields)
  + tasks.archived    ncdb/boolean
  + tasks.assignee    ncdb/link -> people
  + tasks.estimate    ncdb/float
  + tasks.notesAboutAssignee ncdb/string, personal to tasks.assignee
  + tasks.tags        ncdb/set of ncdb/string
  + index tasks.byPriorityDesc (priority desc)
additive: clients on model version 1 keep every change written under version 2.
"
    );
}

#[test]
fn a_change_that_is_not_additive_is_refused_with_the_rfc_messages_and_writes_nothing() {
    let dir = project();
    let schema = SCHEMA_V2
        .replace(
            "priority: nc.int(),",
            r#"priority: nc.oneOf(["low", "high"]),"#,
        )
        .replace("title: nc.string()", "summary: nc.string()");
    write_schema(&dir.join("model.ts"), &schema);
    fs::write(dir.join("model.lock.json"), EXAMPLE).unwrap();
    let run = ncdb(&dir, &["model", "build"]);
    assert_eq!(run.code, 1);
    assert_eq!(
        run.stderr,
        "model.lock.json: refused, the change is not additive (model version stays 2)
  x tasks.priority: type changes from ncdb/int to ncdb/choice.
    Clients would merge and read existing values differently.
    Add a new field instead, and deprecate tasks.priority.
  x tasks.title: removed. Changes name fields; data in tasks.title would belong to nothing.
    To stop using it, keep it in the schema with .deprecated(\"...\"). To rename it, add tasks.summary.
"
    );
    assert_eq!(read(dir.join("model.lock.json")), EXAMPLE);
}

#[test]
fn an_unchanged_schema_is_up_to_date_and_a_description_keeps_the_version() {
    let dir = project();
    write_schema(&dir.join("model.ts"), SCHEMA_V2);
    fs::write(dir.join("model.lock.json"), EXAMPLE).unwrap();
    let run = ncdb(&dir, &["model", "build"]);
    assert_eq!(
        (run.code, run.stdout.as_str()),
        (0, "model.lock.json: up to date (model version 2)\n")
    );
    assert_eq!(read(dir.join("model.lock.json")), EXAMPLE);

    write_schema(
        &dir.join("model.ts"),
        &SCHEMA_V2.replace("One line; what has to be done.", "What to do."),
    );
    let run = ncdb(&dir, &["model", "build"]);
    assert_eq!(
        (run.code, run.stdout.as_str()),
        (
            0,
            "model.lock.json: descriptions changed (model version stays 2)\n  ~ tasks.title       description\n"
        )
    );
    assert!(read(dir.join("model.lock.json")).contains("\"description\": \"What to do.\""));
}

#[test]
fn other_accepted_changes_are_listed() {
    let dir = project();
    write_schema(
        &dir.join("model.ts"),
        &SCHEMA_V2
            .replace(
                "priority: nc.int(),",
                "priority: nc.int().deprecated(\"Use rank.\"),",
            )
            .replace(
                r#"nc.oneOf(["open", "in_progress", "done"])"#,
                r#"nc.oneOf(["open", "in_progress", "done", "blocked"])"#,
            )
            .replace(
                "byStatusPriority: nc.index([\"status\", \"priority\"], { equal: 1 }),",
                "byStatus: nc.index([\"status\"], { equal: 1 }),",
            )
            .replace("nc.link(\"people\")", "nc.link([\"people\", \"tasks\"])"),
    );
    fs::write(dir.join("model.lock.json"), EXAMPLE).unwrap();
    let run = ncdb(&dir, &["model", "build"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "model.lock.json: model version 2 -> 3
  + tasks.assignee    targets tasks
  + tasks.status      options blocked
  ~ tasks.priority    deprecated: Use rank.
  + index tasks.byStatus (status; equal 1)
  - index tasks.byStatusPriority
additive: clients on model version 2 keep every change written under version 3.
"
    );
}

#[test]
fn deprecating_a_collection_writes_it_into_the_lock_and_lists_it() {
    let dir = project();
    let schema = SCHEMA_V2.replace(
        "email: nc.string().personal({ person: \"self\" }),\n  }),",
        "email: nc.string().personal({ person: \"self\" }),\n  }).deprecated(\"Use members.\"),",
    );
    assert_ne!(schema, SCHEMA_V2);
    write_schema(&dir.join("model.ts"), &schema);
    fs::write(dir.join("model.lock.json"), EXAMPLE).unwrap();
    let run = ncdb(&dir, &["model", "build"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "model.lock.json: model version 2 -> 3
  ~ collection people deprecated: Use members.
additive: clients on model version 2 keep every change written under version 3.
"
    );
    let lock = ModelLock::from_json(&read(dir.join("model.lock.json"))).unwrap();
    let deprecation = lock
        .collection("people")
        .unwrap()
        .deprecated
        .clone()
        .unwrap();
    assert_eq!(
        (deprecation.since, deprecation.reason.as_str()),
        (3, "Use members.")
    );
}

#[test]
fn check_passes_on_an_up_to_date_lock_and_writes_nothing() {
    let dir = project();
    write_schema(&dir.join("model.ts"), SCHEMA_V2);
    fs::write(dir.join("model.lock.json"), EXAMPLE).unwrap();
    let run = ncdb(&dir, &["model", "check"]);
    assert_eq!(
        (run.code, run.stdout.as_str()),
        (0, "model.lock.json: up to date (model version 2)\n")
    );
}

#[test]
fn check_fails_on_a_lock_that_is_out_of_date_or_missing_and_writes_nothing() {
    let dir = project();
    write_schema(&dir.join("model.ts"), SCHEMA_V2);
    fs::write(dir.join("model.lock.json"), lock_v1()).unwrap();
    let run = ncdb(&dir, &["model", "check"]);
    assert_eq!(run.code, 1);
    assert!(run.stderr.starts_with("model.lock.json: out of date; run ncdb model build. It would write model version 1 -> 2\n  + collection people"), "{}", run.stderr);
    assert_eq!(read(dir.join("model.lock.json")), lock_v1());

    // A lock in another form than the canonical one is out of date too.
    fs::write(dir.join("model.lock.json"), EXAMPLE.replace("\n", "\r\n")).unwrap();
    let run = ncdb(&dir, &["model", "check"]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr
            .starts_with("model.lock.json: out of date; run ncdb model build."),
        "{}",
        run.stderr
    );

    fs::remove_file(dir.join("model.lock.json")).unwrap();
    let run = ncdb(&dir, &["model", "check"]);
    assert_eq!(
        (run.code, run.stderr.as_str()),
        (1, "model.lock.json: missing; run ncdb model build.\n")
    );
    assert!(!dir.join("model.lock.json").exists());
}

#[test]
fn check_fails_on_a_refused_change() {
    let dir = project();
    write_schema(
        &dir.join("model.ts"),
        &SCHEMA_V2.replace("archived: nc.boolean(),", ""),
    );
    fs::write(dir.join("model.lock.json"), EXAMPLE).unwrap();
    let run = ncdb(&dir, &["model", "check"]);
    assert_eq!(run.code, 1);
    assert!(run.stderr.starts_with("model.lock.json: refused, the change is not additive (model version stays 2)\n  x tasks.archived: removed."), "{}", run.stderr);
}

#[test]
fn model_and_lock_take_their_paths() {
    let dir = project();
    write_schema(&dir.join("src/model.ts"), SCHEMA_V1);
    let run = ncdb(
        &dir,
        &[
            "model",
            "build",
            "--model",
            "src/model.ts",
            "--lock",
            "src/model.lock.json",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        run.stdout
            .starts_with("src/model.lock.json: model version 1, a new model")
    );
    assert!(dir.join("src/model.lock.json").exists() && !dir.join("model.lock.json").exists());
    let run = ncdb(
        &dir,
        &[
            "model",
            "check",
            "--lock",
            "src/model.lock.json",
            "--model",
            "src/model.ts",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
}

#[test]
fn a_lock_edited_by_hand_is_not_built_on() {
    let dir = project();
    write_schema(&dir.join("model.ts"), SCHEMA_V2);
    fs::write(
        dir.join("model.lock.json"),
        EXAMPLE.replace("\"ncdb/float\"", "\"ncdb/int\""),
    )
    .unwrap();
    let run = ncdb(&dir, &["model", "build"]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr
            .starts_with("model.lock.json: its model.hash does not match its content"),
        "{}",
        run.stderr
    );
    fs::write(dir.join("model.lock.json"), "{").unwrap();
    let run = ncdb(&dir, &["model", "build"]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr
            .starts_with("model.lock.json: cannot be read: not valid JSON"),
        "{}",
        run.stderr
    );
}

#[test]
fn a_schema_that_cannot_be_described_is_reported() {
    let dir = project();
    write_schema(
        &dir.join("model.ts"),
        &SCHEMA_V1.replace(
            "priority: nc.int(),",
            "priority: nc.int(), body: nc.richText(),",
        ),
    );
    let run = ncdb(&dir, &["model", "build"]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr
            .contains("model.ts: tasks.body: nc.richText is reserved"),
        "{}",
        run.stderr
    );
    assert!(!dir.join("model.lock.json").exists());

    write_schema(&dir.join("model.ts"), "export const other = 1;");
    let run = ncdb(&dir, &["model", "build"]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr.contains("model.ts: exports no model"),
        "{}",
        run.stderr
    );

    let run = ncdb(&dir, &["model", "build", "--model", "missing.ts"]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr.starts_with("missing.ts: no such file"),
        "{}",
        run.stderr
    );
}

#[test]
fn without_node_it_says_what_it_needs() {
    let dir = project();
    write_schema(&dir.join("model.ts"), SCHEMA_V1);
    let run = ncdb_with(&dir, &["model", "build"], |c| {
        c.env("PATH", "");
    });
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("Node.js 24 or later"), "{}", run.stderr);
}

#[cfg(unix)]
#[test]
fn a_node_older_than_24_is_refused_with_its_version() {
    let dir = project();
    write_schema(&dir.join("model.ts"), SCHEMA_V1);
    let bin = fake_node(&dir, "v22.12.0");
    let run = ncdb_with(&dir, &["model", "build"], |c| {
        c.env("PATH", &bin);
    });
    assert_eq!(run.code, 1);
    assert!(
        run.stderr.contains("Node.js 24 or later") && run.stderr.contains("v22.12.0"),
        "{}",
        run.stderr
    );
    assert!(!dir.join("model.lock.json").exists());
}

/// Many threads write a fake node and run it while the others spawn
/// processes: a forked child must never hold the script open for writing
/// when it runs (ETXTBSY, NOC-94).
#[cfg(unix)]
#[test]
fn a_fake_node_runs_while_other_threads_spawn() {
    let threads: Vec<_> = (0..16)
        .map(|_| {
            std::thread::spawn(|| {
                for _ in 0..25 {
                    let dir = project();
                    let bin = fake_node(&dir, "v22.12.0");
                    let output = Command::new(bin.join("node")).output().unwrap();
                    assert_eq!(String::from_utf8(output.stdout).unwrap(), "v22.12.0\n");
                    fs::remove_dir_all(&dir).unwrap();
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
}

#[test]
fn usage_errors_exit_2() {
    let dir = project();
    for args in [
        &[][..],
        &["model"],
        &["model", "drop"],
        &["model", "build", "--lock"],
        &["model", "build", "--force"],
        &["serve"],
    ] {
        let run = ncdb(&dir, args);
        assert_eq!(run.code, 2, "{args:?}");
        assert!(
            run.stderr.contains("usage: ncdb model build"),
            "{args:?}: {}",
            run.stderr
        );
    }
    let run = ncdb(&dir, &["--help"]);
    assert_eq!(run.code, 0);
    assert!(run.stdout.contains("usage: ncdb model build"));
}

#[test]
fn an_index_in_one_space_is_written_with_its_scope_and_the_option_comes_and_goes_as_a_change() {
    let dir = project();
    let scoped = SCHEMA_V2.replace(
        r#"nc.index([{ field: "priority", direction: "desc" }])"#,
        r#"nc.index([{ field: "priority", direction: "desc" }], { scope: "space" })"#,
    );
    write_schema(&dir.join("model.ts"), &scoped);
    fs::write(dir.join("model.lock.json"), EXAMPLE).unwrap();
    let run = ncdb(&dir, &["model", "build"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "model.lock.json: model version 2 -> 3
  ~ index tasks.byPriorityDesc (priority desc; scope space)
additive: clients on model version 2 keep every change written under version 3.
"
    );
    let written = read(dir.join("model.lock.json"));
    assert_eq!(
        written.matches(r#""scope": "space""#).count(),
        1,
        "{written}"
    );
    let lock = ModelLock::from_json(&written).unwrap();
    let tasks = lock.collection("tasks").unwrap();
    let scoped_index = &tasks.indexes["byPriorityDesc"];
    assert_eq!((scoped_index.scope, scoped_index.since), (Scope::Space, 3));
    let other = &tasks.indexes["byStatusPriority"];
    assert_eq!((other.scope, other.since), (Scope::Member, 1));
    assert_eq!(
        ncdb(&dir, &["model", "check"]).stdout,
        "model.lock.json: up to date (model version 3)\n"
    );

    // Leaving the option out again is a change to the index too.
    write_schema(&dir.join("model.ts"), SCHEMA_V2);
    let run = ncdb(&dir, &["model", "build"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "model.lock.json: model version 3 -> 4
  ~ index tasks.byPriorityDesc (priority desc)
additive: clients on model version 3 keep every change written under version 4.
"
    );
    let written = read(dir.join("model.lock.json"));
    assert!(!written.contains("\"scope\""), "{written}");
    let lock = ModelLock::from_json(&written).unwrap();
    let index = &lock.collection("tasks").unwrap().indexes["byPriorityDesc"];
    assert_eq!((index.scope, index.since), (Scope::Member, 4));
}

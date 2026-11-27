//! `ncdb model build` on `nc.ref` where a link is locked (RFC 0018, "One
//! function, two cases" and "Existing link fields read as references"), run
//! as a developer runs it: on a TypeScript schema, through Node.js, in a
//! project directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use ncdb_core::model_lock::{Kind, ModelLock};

/// RFC 0005's version 2, whose lock has `tasks.assignee` as `ncdb/link`.
const EXAMPLE: &str = include_str!("../../ncdb-core/tests/model_lock/rfc-0005-v2.lock.json");

/// RFC 0005's version 2 schema, with `assignee` written as `assignee`.
fn schema(assignee: &str) -> String {
    format!(
        r#"export const model = nc.model({{
  people: nc.collection({{
    name: nc.string().personal({{ person: "self" }}),
    email: nc.string().personal({{ person: "self" }}),
  }}),
  tasks: nc.collection(
    {{
      title: nc.string().describe("One line; what has to be done."),
      status: nc.oneOf(["open", "in_progress", "done"]),
      priority: nc.int(),
      estimate: nc.float(),
      archived: nc.boolean(),
      tags: nc.set(nc.string()),
      assignee: {assignee},
      notesAboutAssignee: nc.string().personal({{ person: "assignee" }}),
      dependsOn: nc.links("tasks"),
    }},
    {{
      computed: {{ blocked: nc.reach("dependsOn", {{ status: ["open", "in_progress"] }}) }},
      views: {{ ready: nc.view("tasks", {{ where: {{ blocked: false }}, sort: ["priority"] }}) }},
      indexes: {{
        byStatusPriority: nc.index(["status", "priority"], {{ equal: 1 }}),
        byPriorityDesc: nc.index([{{ field: "priority", direction: "desc" }}]),
      }},
    }},
  ),
}});
"#
    )
}

fn project() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ncdb-model-ref-link-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes a schema module that imports the SDK of this repository.
fn write_schema(dir: &Path, body: &str) {
    let sdk = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/sdk/src/index.ts");
    let sdk = fs::canonicalize(sdk).unwrap();
    fs::write(
        dir.join("model.ts"),
        format!("import {{ nc }} from {:?};\n{body}", sdk.to_str().unwrap()),
    )
    .unwrap();
}

/// Runs `ncdb model build` in `dir`: its exit code and its output.
fn build(dir: &Path) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ncdb"))
        .args(["model", "build"])
        .current_dir(dir)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.code().unwrap(), text)
}

#[test]
fn nc_ref_over_a_field_locked_as_a_link_is_up_to_date_and_keeps_the_lock() {
    let dir = project();
    write_schema(&dir, &schema(r#"nc.ref("people")"#));
    fs::write(dir.join("model.lock.json"), EXAMPLE).unwrap();
    assert_eq!(
        build(&dir),
        (
            0,
            "model.lock.json: up to date (model version 2)\n".to_owned()
        )
    );
    assert_eq!(
        fs::read_to_string(dir.join("model.lock.json")).unwrap(),
        EXAMPLE
    );
}

#[test]
fn a_new_reference_that_is_the_person_of_a_personal_field_is_locked_as_a_link() {
    let dir = project();
    write_schema(&dir, &schema(r#"nc.ref("people")"#));
    let (code, text) = build(&dir);
    assert_eq!(code, 0, "{text}");
    let lock =
        ModelLock::from_json(&fs::read_to_string(dir.join("model.lock.json")).unwrap()).unwrap();
    assert_eq!(
        lock.field("tasks", "assignee").unwrap().kind,
        Kind::Link(vec!["people".into()])
    );
}

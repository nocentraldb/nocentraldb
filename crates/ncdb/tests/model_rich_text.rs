//! `ncdb model build` on a schema with `nc.richText()` (RFC 0013, "The
//! field in the schema and the lock"), run as a developer runs it: on a
//! TypeScript schema, through Node.js, in a project directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use ncdb_core::model_lock::{Kind, ModelLock};

/// RFC 0013's notes, with the body when `body` is true.
fn schema(body: bool) -> String {
    let body = if body { "body: nc.richText()," } else { "" };
    format!(
        r#"export const model = nc.model({{
  notes: nc.collection({{ title: nc.string(), {body} }}),
}});
"#
    )
}

fn project() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ncdb-model-rich-text-test-{}-{}",
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

/// Runs `ncdb model build` in `dir`, and returns its stdout.
fn build(dir: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_ncdb"))
        .args(["model", "build"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// The type, rule and `since` of `notes.body` in the lock the build wrote.
fn body(dir: &Path) -> (Kind, u64, u64) {
    let text = fs::read_to_string(dir.join("model.lock.json")).unwrap();
    let lock = ModelLock::from_json(&text).expect("the device reads the lock");
    assert!(text.contains(r#""type": "ncdb/richText""#), "{text}");
    let body = lock
        .field("notes", "body")
        .expect("the lock has notes.body");
    (body.kind.clone(), body.rule, body.since)
}

#[test]
fn model_build_writes_a_rich_text_field_into_the_lock() {
    let dir = project();
    write_schema(&dir, &schema(true));
    build(&dir);
    assert_eq!(body(&dir), (Kind::RichText, 1, 1));
}

#[test]
fn adding_a_rich_text_field_is_additive_and_names_its_version() {
    let dir = project();
    write_schema(&dir, &schema(false));
    build(&dir);
    write_schema(&dir, &schema(true));
    let stdout = build(&dir);
    assert!(
        stdout.contains("+ notes.body") && stdout.contains("ncdb/richText"),
        "{stdout}"
    );
    assert_eq!(body(&dir), (Kind::RichText, 1, 2));
}

/// RFC 0013's notes, with a body whose vocabulary is `nodes` and `marks`.
fn schema_with(nodes: &[&str], marks: &[&str]) -> String {
    format!(
        r#"export const model = nc.model({{
  notes: nc.collection({{
    title: nc.string(),
    body: nc.richText({{ vocabulary: {{ nodes: {nodes:?}, marks: {marks:?} }} }}),
  }}),
}});
"#
    )
}

/// Runs `ncdb model <command>` in `dir`, and returns whether it passed and
/// what it printed.
fn run(dir: &Path, command: &str) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ncdb"))
        .args(["model", command])
        .current_dir(dir)
        .output()
        .unwrap();
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.success(), text)
}

#[test]
fn model_build_writes_the_vocabulary_sorted() {
    let dir = project();
    write_schema(
        &dir,
        &schema_with(&["paragraph", "callout"], &["link", "bold"]),
    );
    build(&dir);
    let text = fs::read_to_string(dir.join("model.lock.json")).unwrap();
    let lock = ModelLock::from_json(&text).expect("the device reads the lock");
    let vocabulary = &lock.field("notes", "body").unwrap().vocabulary;
    assert_eq!(
        (
            vocabulary
                .nodes
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vocabulary
                .marks
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        ),
        (vec!["callout", "paragraph"], vec!["bold", "link"])
    );
    let at = |name: &str| text.find(&format!("{name:?}")).unwrap();
    assert!(
        at("callout") < at("paragraph") && at("bold") < at("link"),
        "{text}"
    );
    let (passed, text) = run(&dir, "check");
    assert!(passed, "{text}");
}

#[test]
fn model_build_accepts_added_names_and_lists_them() {
    let dir = project();
    write_schema(&dir, &schema_with(&["paragraph"], &["bold"]));
    build(&dir);
    write_schema(
        &dir,
        &schema_with(&["paragraph", "callout"], &["bold", "link"]),
    );
    let stdout = build(&dir);
    assert!(stdout.contains("model version 1 -> 2"), "{stdout}");
    assert!(
        stdout.contains("+ notes.body") && stdout.contains("nodes callout"),
        "{stdout}"
    );
    assert!(stdout.contains("marks link"), "{stdout}");
}

#[test]
fn model_build_and_check_refuse_a_removed_name_and_write_nothing() {
    let dir = project();
    write_schema(
        &dir,
        &schema_with(&["paragraph", "callout"], &["bold", "link"]),
    );
    build(&dir);
    let before = fs::read_to_string(dir.join("model.lock.json")).unwrap();
    write_schema(&dir, &schema_with(&["paragraph"], &["bold"]));
    let (passed, text) = run(&dir, "build");
    assert!(!passed, "{text}");
    assert!(
        text.contains("x notes.body") && text.contains("callout"),
        "{text}"
    );
    assert!(text.contains("link"), "{text}");
    assert_eq!(
        fs::read_to_string(dir.join("model.lock.json")).unwrap(),
        before
    );
    let (passed, text) = run(&dir, "check");
    assert!(!passed, "{text}");
}

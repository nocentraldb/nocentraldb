//! A reader that closes `tasks`'s output early, as `tasks … invitation |
//! head -1` does after the first of its two lines (README step 3), ends the
//! app quietly: no panic, no message, exit status 0 (NOC-340).

use std::path::PathBuf;
use std::process::{Command, Stdio};

/// A directory of its own for one run of the test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("tasks-pipe-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs `tasks` with `args` while nothing reads its output: the read end
/// of its stdout is closed before the app writes, so every write meets a
/// broken pipe. Returns the exit code and what it wrote to stderr.
fn with_stdout_closed(args: &[&str]) -> (Option<i32>, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tasks"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("tasks starts");
    drop(child.stdout.take());
    let output = child.wait_with_output().expect("tasks ends");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn invitation_ends_quietly_when_its_reader_closes_stdout() {
    let dir = TempDir::new();
    let data = dir.0.join("data");
    let keys = dir.0.join("keys");
    let (code, stderr) = with_stdout_closed(&[
        "--data",
        data.to_str().unwrap(),
        "--key-file",
        keys.to_str().unwrap(),
        "invitation",
    ]);
    assert_eq!(stderr, "", "nothing on stderr, no panic");
    assert_eq!(code, Some(0));
}

#[test]
fn help_ends_quietly_when_its_reader_closes_stdout() {
    let (code, stderr) = with_stdout_closed(&["--help"]);
    assert_eq!(stderr, "", "nothing on stderr, no panic");
    assert_eq!(code, Some(0));
}

//! The first device's side of the second device (`catch_up.rs`): starting
//! its process, pairing it, and reading what it reports.

use std::io::{BufRead, BufReader, Lines, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use nocentraldb::{Db, Invitation, ObjectId, PairKind, SpaceId};

/// The second device's process.
pub struct Second {
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
}

/// What the second device reported once it caught up.
pub struct CaughtUp {
    pub millis: u64,
    pub items: u64,
    pub file: u64,
    pub wal: u64,
    pub peak: u64,
}

impl Second {
    /// Starts `exe catch-up <dir> <relay> <mode>`.
    pub fn start(exe: &Path, dir: &Path, relay: &str, mode: &str) -> Result<Second, String> {
        let mut child = Command::new(exe)
            .arg("catch-up")
            .arg(dir)
            .args([relay, mode])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let stdin = child.stdin.take().ok_or("no input")?;
        let lines = BufReader::new(child.stdout.take().ok_or("no output")?).lines();
        Ok(Second {
            child,
            stdin,
            lines,
        })
    }

    fn line(&mut self) -> Result<String, String> {
        self.lines
            .next()
            .ok_or("the second device ended early")?
            .map_err(|e| e.to_string())
    }

    fn send(&mut self, line: &str) -> Result<(), String> {
        writeln!(self.stdin, "{line}").map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())
    }

    /// Pairs the new device with `first`, which holds the account key, and
    /// waits until it joined `space`.
    pub async fn pair(&mut self, first: &Db, space: SpaceId) -> Result<(), String> {
        let line = self.line()?;
        let text = line
            .strip_prefix("invitation ")
            .ok_or_else(|| format!("unexpected line: {line}"))?;
        let invitation: Invitation = text.parse().map_err(|e| format!("{e}"))?;
        // In an app, a person compares the fingerprint on both screens first.
        let paired = first
            .account()
            .pair(PairKind::Device, &invitation)
            .await
            .map_err(|e| e.to_string())?;
        self.send(&format!("answer {} {space}", paired.answer))?;
        match self.line()?.as_str() {
            "joined" => Ok(()),
            line => Err(format!("unexpected line: {line}")),
        }
    }

    /// Ends the device's process; its database stays.
    pub fn leave(mut self) -> Result<(), String> {
        self.send("leave")?;
        self.child.wait().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Waits until the device holds the item `last` of `space`.
    pub fn caught_up(mut self, space: SpaceId, last: ObjectId) -> Result<CaughtUp, String> {
        self.send(&format!("wait {space} {last}"))?;
        let line = self.line()?;
        let numbers: Vec<u64> = line
            .strip_prefix("caught-up ")
            .ok_or_else(|| format!("unexpected line: {line}"))?
            .split_whitespace()
            .map(|n| n.parse().map_err(|_| format!("unexpected line: {line}")))
            .collect::<Result<_, _>>()?;
        let [millis, items, file, wal, peak] = numbers[..] else {
            return Err(format!("unexpected line: {line}"));
        };
        let _ = self.child.wait();
        Ok(CaughtUp {
            millis,
            items,
            file,
            wal,
            peak,
        })
    }
}

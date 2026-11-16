// Holds the session start against a stale checkout: the project's
// SessionStart hooks run scripts/session-start-git.mjs (NOC-52) and then
// scripts/session-start-deps.mjs (NOC-65), and the coding agent's role tells
// it to check `git status` first and stash what it did not write, and to run
// `pnpm install` once when a test fails with a missing module.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

test("the first SessionStart hook runs session-start-git.mjs", () => {
  const settings = JSON.parse(read(".claude/settings.json"));
  const [first] = settings.hooks.SessionStart.flatMap((entry) => entry.hooks);
  assert.equal(first.type, "command");
  assert.match(first.command, /scripts\/session-start-git\.mjs/);
});

// A restored session's working tree is the snapshot's, so its copy of the
// script is as old as the snapshot; a fix merged since then would never run
// (NOC-350). The hook runs the script as HEAD holds it.
test("the first SessionStart hook runs HEAD's script, not the working tree's", () => {
  const settings = JSON.parse(read(".claude/settings.json"));
  const [first] = settings.hooks.SessionStart.flatMap((entry) => entry.hooks);
  const dir = mkdtempSync(join(tmpdir(), "session-start-wiring-"));
  try {
    const git = (...args) =>
      spawnSync(
        "git",
        ["-c", "user.email=t@example.com", "-c", "user.name=T", ...args],
        {
          cwd: dir,
        },
      );
    git("init", "-q");
    mkdirSync(join(dir, "scripts"));
    const path = join(dir, "scripts", "session-start-git.mjs");
    writeFileSync(path, 'console.log("head");\n');
    git("add", ".");
    git("commit", "-q", "-m", "head");
    writeFileSync(path, 'console.log("stale");\n');
    git("add", ".");
    const run = spawnSync("bash", ["-c", first.command], {
      env: { ...process.env, CLAUDE_PROJECT_DIR: dir },
      input: JSON.stringify({ source: "startup" }),
      encoding: "utf8",
    });
    assert.equal(run.stdout.trim(), "head");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("coder.md starts with git status and a stash of what the agent did not write", () => {
  const coder = read("factory/roles/coder.md");
  const start = coder.split("## Before anything else")[1].split("\n## ")[0];
  assert.match(start, /`git status`/);
  assert.match(start, /stash/);
  assert.match(start, /did not write/);
});

test("the second SessionStart hook installs dependencies, after the git repair", () => {
  const settings = JSON.parse(read(".claude/settings.json"));
  const [, second] = settings.hooks.SessionStart.flatMap(
    (entry) => entry.hooks,
  );
  assert.equal(second.type, "command");
  assert.match(second.command, /node scripts\/session-start-deps\.mjs/);
  assert.ok(second.timeout >= 300, "leaves pnpm install time to finish");
});

test("coder.md says to run pnpm install once when a test misses a module", () => {
  const coder = read("factory/roles/coder.md");
  const start = coder.split("## Before anything else")[1].split("\n## ")[0];
  assert.match(start, /`pnpm install`/);
  assert.match(start, /missing module/);
});

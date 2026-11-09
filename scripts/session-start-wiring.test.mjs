// Holds the session start against a stale checkout: the project's
// SessionStart hooks run scripts/session-start-git.mjs (NOC-52) and then
// scripts/session-start-deps.mjs (NOC-65), and the coding agent's role tells
// it to check `git status` first and stash what it did not write, and to run
// `pnpm install` once when a test fails with a missing module.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

test("the first SessionStart hook runs session-start-git.mjs", () => {
  const settings = JSON.parse(read(".claude/settings.json"));
  const [first] = settings.hooks.SessionStart.flatMap((entry) => entry.hooks);
  assert.equal(first.type, "command");
  assert.match(first.command, /node scripts\/session-start-git\.mjs/);
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

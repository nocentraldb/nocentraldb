// Holds the session start against a stale checkout (NOC-52): the project's
// SessionStart hook runs scripts/session-start-git.mjs, and the coding agent's
// role tells it to check `git status` first and stash what it did not write.
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

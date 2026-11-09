// Runs session-start-git.mjs in throwaway git repositories that reproduce how
// a cloud session starts (NOC-52): the container is restored from a snapshot
// whose checkout is an old commit, then HEAD is moved to the fetched main
// without touching the index or the working tree, and the ticket branch is
// checked out from there. The old tree then sits staged over HEAD. The script
// puts the checkout back on HEAD, but only when that loses nothing.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  appendFileSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

const script = join(
  dirname(fileURLToPath(import.meta.url)),
  "session-start-git.mjs",
);
const fixtures = [];
after(() => {
  for (const dir of fixtures) rmSync(dir, { recursive: true, force: true });
});

const CLOUD =
  "\n<!-- BEGIN CLOUD: added by the setup script, never commit -->\ncloud steering\n<!-- END CLOUD -->\n";

function git(cwd, ...args) {
  const run = spawnSync("git", args, { cwd, encoding: "utf8" });
  assert.equal(run.status, 0, `git ${args.join(" ")}: ${run.stderr}`);
  return run.stdout.trim();
}

function commit(cwd, files, message) {
  for (const [name, content] of Object.entries(files)) {
    if (content === null) git(cwd, "rm", "-q", name);
    else {
      writeFileSync(join(cwd, name), content);
      git(cwd, "add", name);
    }
  }
  git(cwd, "commit", "-q", "-m", message);
  return git(cwd, "rev-parse", "HEAD");
}

/**
 * A repository as a restored cloud session finds it: the snapshot checked out
 * `old`, the setup script appended the cloud block to AGENTS.md and marked it
 * skip-worktree, then HEAD moved to `head` and the ticket branch was created.
 */
function staleSession() {
  const dir = mkdtempSync(join(tmpdir(), "session-start-git-"));
  fixtures.push(dir);
  git(dir, "init", "-q", "-b", "main");
  git(dir, "config", "user.email", "test@example.com");
  git(dir, "config", "user.name", "Test");
  const old = commit(
    dir,
    { "AGENTS.md": "old steering\n", "gone.txt": "removed later\n" },
    "old",
  );
  const head = commit(
    dir,
    { "AGENTS.md": "new steering\n", "gone.txt": null, "new.txt": "added\n" },
    "new",
  );
  git(dir, "reset", "-q", "--hard", old);
  appendFileSync(join(dir, "AGENTS.md"), CLOUD);
  git(dir, "update-index", "--skip-worktree", "AGENTS.md");
  git(dir, "checkout", "-q", "--detach");
  git(dir, "update-ref", "--no-deref", "HEAD", head);
  git(dir, "checkout", "-q", "-b", "ticket");
  return { dir, old, head };
}

function run(dir, env = { CLAUDE_CODE_REMOTE: "true" }) {
  const result = spawnSync(process.execPath, [script], {
    cwd: dir,
    encoding: "utf8",
    env: { PATH: process.env.PATH, HOME: process.env.HOME, ...env },
  });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout + result.stderr;
}

const status = (dir) => git(dir, "status", "--porcelain");

test("the fixture reproduces the stale index of NOC-52", () => {
  const { dir } = staleSession();
  assert.match(status(dir), /^M {2}AGENTS\.md$/m);
  assert.match(status(dir), /^A {2}gone\.txt$/m);
  assert.match(status(dir), /^D {2}new\.txt$/m);
});

test("a stale tree from an older commit is reset to HEAD", () => {
  const { dir, old } = staleSession();
  const out = run(dir);
  assert.equal(status(dir), "");
  assert.equal(readFileSync(join(dir, "new.txt"), "utf8"), "added\n");
  assert.equal(existsSync(join(dir, "gone.txt")), false);
  assert.ok(out.includes(old), `names the old commit: ${out}`);
});

test("the cloud block survives on top of HEAD's AGENTS.md, hidden from git", () => {
  const { dir } = staleSession();
  run(dir);
  assert.equal(
    readFileSync(join(dir, "AGENTS.md"), "utf8"),
    `new steering\n${CLOUD}`,
  );
  assert.equal(git(dir, "ls-files", "-v", "AGENTS.md"), "S AGENTS.md");
});

test("staged work that matches no commit is left alone", () => {
  const { dir } = staleSession();
  writeFileSync(join(dir, "mine.txt"), "an agent's own work\n");
  git(dir, "add", "mine.txt");
  const before = status(dir);
  const out = run(dir);
  assert.equal(status(dir), before);
  assert.match(out, /git status/);
});

test("unstaged edits over the stale tree are left alone", () => {
  const { dir } = staleSession();
  writeFileSync(join(dir, "gone.txt"), "edited\n");
  const before = status(dir);
  run(dir);
  assert.equal(status(dir), before);
});

test("untracked files are left alone", () => {
  const { dir } = staleSession();
  writeFileSync(join(dir, "untracked.txt"), "keep me\n");
  const before = status(dir);
  run(dir);
  assert.equal(status(dir), before);
});

test("outside a cloud session the script changes nothing", () => {
  const { dir } = staleSession();
  const before = status(dir);
  run(dir, {});
  assert.equal(status(dir), before);
});

test("a checkout already on HEAD stays as it is, silently", () => {
  const { dir } = staleSession();
  run(dir);
  assert.equal(run(dir), "");
  assert.equal(status(dir), "");
});

test("a hidden file missing from disk is left alone, without failing", () => {
  const { dir } = staleSession();
  rmSync(join(dir, "AGENTS.md"));
  const before = status(dir);
  const out = run(dir);
  assert.equal(status(dir), before);
  assert.match(out, /AGENTS\.md/);
});

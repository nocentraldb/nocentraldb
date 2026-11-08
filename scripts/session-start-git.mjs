// Puts a cloud session's checkout back on HEAD at session start (NOC-52).
//
// A cloud session's container is restored from a snapshot taken when the
// environment was built. Its checkout is the commit of that day. At session
// start, HEAD is moved to the freshly fetched main without updating the index
// or the working tree, and the ticket branch is checked out from there. The
// old tree then sits staged over HEAD, ready to be committed as a revert.
//
// This script resets index and working tree to HEAD, but only when that loses
// nothing: the index holds exactly the tree of a commit in HEAD's history, the
// working tree matches the index, no file is untracked, and every
// skip-worktree file (AGENTS.md with the setup script's cloud block) is its
// indexed content plus an addition, which is kept on top of HEAD's version.
// In any other case it changes nothing and says so. It runs only in cloud
// sessions (CLAUDE_CODE_REMOTE=true) and always exits 0, so it cannot block a
// session from starting.
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const NAME = "session-start-git";

function git(...args) {
  const run = spawnSync("git", args, { encoding: "utf8" });
  return { ok: run.status === 0, raw: run.stdout, out: run.stdout.trimEnd() };
}

function leave(reason) {
  console.log(
    `${NAME}: the index differs from HEAD and was left alone (${reason}). Check \`git status\` and stash anything you did not write.`,
  );
  process.exit(0);
}

if (process.env.CLAUDE_CODE_REMOTE !== "true") process.exit(0);

const head = git("rev-parse", "HEAD^{tree}");
const index = git("write-tree");
if (!head.ok || !index.ok) process.exit(0);
if (head.out === index.out) process.exit(0);

const commits = git("log", "--format=%H %T", "HEAD").out.split("\n");
const old = commits.find((line) => line.endsWith(` ${index.out}`));
if (!old) leave("its tree is no commit in HEAD's history");
const oldCommit = old.split(" ")[0];

if (!git("diff", "--quiet").ok) leave("the working tree has unstaged changes");
if (git("ls-files", "--others", "--exclude-standard").out !== "") {
  leave("there are untracked files");
}

// Each skip-worktree file must be its indexed content plus an addition.
const hidden = git("ls-files", "-v")
  .out.split("\n")
  .filter((line) => line.startsWith("S "))
  .map((line) => line.slice(2));
const additions = new Map();
for (const file of hidden) {
  const indexed = git("show", `:${file}`).raw;
  const current = readFileSync(file, "utf8");
  if (!current.startsWith(indexed)) {
    leave(`${file} is hidden from git and changed beyond an addition`);
  }
  additions.set(file, current.slice(indexed.length));
}

for (const file of hidden) git("update-index", "--no-skip-worktree", file);
if (!git("reset", "--quiet", "--hard", "HEAD").ok) {
  leave("git reset failed");
}
for (const [file, addition] of additions) {
  writeFileSync(file, readFileSync(file, "utf8") + addition);
  git("update-index", "--skip-worktree", file);
}

console.log(
  `${NAME}: the index and working tree held the tree of ${oldCommit} staged over HEAD; reset them to HEAD. The old tree stays in that commit.`,
);

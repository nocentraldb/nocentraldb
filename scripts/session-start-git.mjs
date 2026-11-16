// Puts a cloud session's checkout back on HEAD at session start (NOC-52).
//
// A cloud session's container is restored from a snapshot taken when the
// environment was built. Its checkout is the commit of that day, and its local
// main still points there. At session start, HEAD is moved to the freshly
// fetched main without updating the index or the working tree, and the ticket
// branch is checked out from there. The old tree then sits staged over HEAD,
// ready to be committed as a revert. The clone is shallow (depth 50), so once
// main is more than 50 commits past the snapshot, HEAD's history no longer
// reaches the snapshot's commit (NOC-350).
//
// The working tree is the snapshot's too, so its copy of this script is as old
// as the snapshot. The hooks in .claude/settings.json therefore run this script
// and session-start-deps.mjs as HEAD holds them, and a fix merged since the
// snapshot takes effect.
//
// This script resets index and working tree to HEAD when that loses nothing:
// the index holds exactly the tree of a commit on HEAD or on a local branch,
// the working tree matches the index, and no file is untracked. At a session's
// startup (the hook's source is "startup") a fresh checkout holds no work of
// its own, so any other stale tree is stashed under a named stash and the
// checkout is reset all the same. Either way every skip-worktree file
// (AGENTS.md with the setup script's cloud block) must be its indexed content
// plus an addition, which is kept on top of HEAD's version. In any other case
// it changes nothing and warns, counting the staged paths. It runs only in
// cloud sessions (CLAUDE_CODE_REMOTE=true) and always exits 0, so it cannot
// block a session from starting.
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";

const NAME = "session-start-git";

function git(...args) {
  const run = spawnSync("git", args, { encoding: "utf8" });
  return { ok: run.status === 0, raw: run.stdout, out: run.stdout.trimEnd() };
}

// The session must not start on a stale tree unnoticed: a commit would carry
// it. The hook's output goes into the agent's context, so it says so plainly.
function leave(reason) {
  const staged = git("diff", "--cached", "--name-only", "HEAD").out;
  const paths = staged === "" ? 0 : staged.split("\n").length;
  console.log(
    `${NAME}: WARNING: ${paths} paths are staged over HEAD and were left alone (${reason}); a commit now would revert them. Check \`git status\` and stash anything you did not write, as factory/roles/coder.md describes.`,
  );
  process.exit(0);
}

if (process.env.CLAUDE_CODE_REMOTE !== "true") process.exit(0);

const head = git("rev-parse", "HEAD^{tree}");
const index = git("write-tree");
if (!head.ok || !index.ok) process.exit(0);
if (head.out === index.out) process.exit(0);

// The hook's input names what started the session; a plain run has none.
function source() {
  try {
    return JSON.parse(readFileSync(0, "utf8")).source;
  } catch {
    return undefined;
  }
}

const commits = git("log", "--format=%H %T", "HEAD", "--branches").out.split(
  "\n",
);
const old = commits.find((line) => line.endsWith(` ${index.out}`));
let unsafe;
if (!old) unsafe = "its tree is no commit on HEAD or a local branch";
else if (!git("diff", "--quiet").ok) {
  unsafe = "the working tree has unstaged changes";
} else if (git("ls-files", "--others", "--exclude-standard").out !== "") {
  unsafe = "there are untracked files";
}
const stash = unsafe !== undefined;
if (stash && source() !== "startup") leave(unsafe);

// Each skip-worktree file must be its indexed content plus an addition.
const hidden = git("ls-files", "-v")
  .out.split("\n")
  .filter((line) => line.startsWith("S "))
  .map((line) => line.slice(2));
const additions = new Map();
for (const file of hidden) {
  const indexed = git("show", `:${file}`).raw;
  const current = existsSync(file) ? readFileSync(file, "utf8") : "";
  if (!existsSync(file) || !current.startsWith(indexed)) {
    leave(`${file} is hidden from git and changed beyond an addition`);
  }
  additions.set(file, current.slice(indexed.length));
}

for (const file of hidden) git("update-index", "--no-skip-worktree", file);
const message = `${NAME}: tree ${index.out} staged over ${git("rev-parse", "--short", "HEAD").out} at session start`;
const repaired = stash
  ? git("stash", "push", "--quiet", "--include-untracked", "-m", message).ok
  : git("reset", "--quiet", "--hard", "HEAD").ok;
if (!repaired) leave(stash ? "git stash failed" : "git reset failed");
for (const [file, addition] of additions) {
  writeFileSync(file, readFileSync(file, "utf8") + addition);
  git("update-index", "--skip-worktree", file);
}

console.log(
  stash
    ? `${NAME}: the checkout held tree ${index.out} staged over HEAD (${unsafe}). A session's startup holds no work of its own, so it was stashed as stash@{0} ("${message}") and the checkout reset to HEAD. Drop the stash with \`git stash drop\` once you have checked it.`
    : `${NAME}: the index and working tree held the tree of ${old.split(" ")[0]} staged over HEAD; reset them to HEAD. The old tree stays in that commit.`,
);

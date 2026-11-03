import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { nothingCommitted, parseMain, pushWithRetry } from "./commit-back.mjs";
import { GIT_ENV, makeRepo } from "./lib/test-repo.mjs";

const script = join(dirname(fileURLToPath(import.meta.url)), "commit-back.mjs");

const fragment = (line) =>
  `---\ntype: internal\ntickets: [mt08.aaaa]\ncomponents: [core]\n---\n${line}\n`;
const EMPTY_INDEX = '{\n  "releases": []\n}\n';
const FINAL_NOTES = "# 0.1.0\n\nFinal notes.\n";
const RELEASE_MESSAGE =
  "Release v0.1.0: index, notes archive and shipped fragments";

const git = (cwd, ...args) =>
  execFileSync("git", args, {
    cwd,
    env: GIT_ENV,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();

/**
 * A bare origin and a clone of its main. On origin: main with fragments a, b
 * and c and an empty index; release/0.1 cut from it with the beta notes,
 * tagged v0.1.0-beta.1, then a and b deleted and the notes made final,
 * tagged v0.1.0; on main meanwhile b rewritten under the same slug.
 * `mainFiles` are committed on main last. The clone has main checked out.
 */
const releasedOrigin = (t, mainFiles = {}) => {
  const author = makeRepo();
  const origin = mkdtempSync(join(tmpdir(), "release-origin-"));
  const clone = mkdtempSync(join(tmpdir(), "release-clone-"));
  t.after(() => {
    author.dispose();
    rmSync(origin, { recursive: true, force: true });
    rmSync(clone, { recursive: true, force: true });
  });
  author.commit("fragments", {
    "changes/README.md": "# guide\n",
    "changes/a.md": fragment("A."),
    "changes/b.md": fragment("B."),
    "changes/c.md": fragment("C."),
    "release-notes/index.json": EMPTY_INDEX,
  });
  author.run("switch", "-q", "-c", "release/0.1");
  author.commit("beta notes", { "release-notes/0.1.0.md": "# 0.1.0\n" });
  author.tag("v0.1.0-beta.1");
  author.remove("changes/a.md");
  author.remove("changes/b.md");
  author.commit("final notes", { "release-notes/0.1.0.md": FINAL_NOTES });
  author.tag("v0.1.0");
  author.run("switch", "-q", "main");
  author.commit("b reused", { "changes/b.md": fragment("Another B.") });
  if (Object.keys(mainFiles).length > 0) author.commit("main", mainFiles);
  git(origin, "init", "-q", "--bare", "-b", "main");
  author.run("push", "-q", origin, "main", "release/0.1", "--tags");
  git(tmpdir(), "clone", "-q", origin, clone);
  git(clone, "config", "commit.gpgsign", "false");
  return { author, origin, clone };
};

const run = (cwd, ...args) =>
  spawnSync(process.execPath, [script, ...args], {
    cwd,
    env: GIT_ENV,
    encoding: "utf8",
  });

const RELEASE = [
  "--tag",
  "v0.1.0",
  "--stage",
  "release",
  "--version",
  "0.1.0",
  "--date",
  "2026-11-02",
];
const BETA = [
  "--tag",
  "v0.1.0-beta.1",
  "--stage",
  "beta",
  "--version",
  "0.1.0",
  "--date",
  "2026-11-02",
];

const entry = (stage, tag) => ({
  version: "0.1.0",
  stage,
  tag,
  date: "2026-11-02",
  path: "release-notes/0.1.0.md",
});
const indexAt = (cwd, ref) =>
  JSON.parse(git(cwd, "show", `${ref}:release-notes/index.json`));

test("a release commits the index entry, the notes as they stand at the tag and the removal of the shipped fragments", (t) => {
  const { origin, clone } = releasedOrigin(t);
  const before = git(clone, "rev-parse", "HEAD");
  const r = run(clone, ...RELEASE, "--no-push");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(git(clone, "rev-parse", "HEAD~1"), before);
  assert.equal(git(clone, "log", "-1", "--format=%s"), RELEASE_MESSAGE);
  assert.equal(
    git(clone, "diff", "--name-status", "HEAD~1", "HEAD"),
    "D\tchanges/a.md\nA\trelease-notes/0.1.0.md\nM\trelease-notes/index.json",
  );
  assert.deepEqual(indexAt(clone, "HEAD"), {
    releases: [entry("release", "v0.1.0")],
  });
  assert.equal(
    git(clone, "show", "HEAD:release-notes/0.1.0.md"),
    FINAL_NOTES.trim(),
  );
  assert.equal(git(clone, "status", "--porcelain"), "");
  // --no-push leaves origin as it was.
  assert.equal(git(origin, "rev-parse", "main"), before);
});

test("a release keeps a fragment on main whose content differs from the shipped one, and still commits", (t) => {
  const { clone } = releasedOrigin(t);
  const r = run(clone, ...RELEASE, "--no-push");
  assert.equal(r.status, 0, r.stderr);
  assert.match(
    r.stdout,
    /kept changes\/b\.md: differs from the shipped fragment/,
  );
  assert.equal(git(clone, "log", "-1", "--format=%s"), RELEASE_MESSAGE);
  assert.equal(
    git(clone, "show", "HEAD:changes/b.md"),
    fragment("Another B.").trim(),
  );
  assert.ok(existsSync(join(clone, "changes/c.md")));
});

test("a second identical run prints nothing to commit and makes no commit", (t) => {
  const { origin, clone } = releasedOrigin(t);
  assert.equal(run(clone, ...RELEASE).status, 0);
  const head = git(clone, "rev-parse", "HEAD");
  const r = run(clone, ...RELEASE);
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /^commit-back: nothing to commit$/m);
  assert.equal(git(clone, "rev-parse", "HEAD"), head);
  assert.equal(git(origin, "rev-parse", "main"), head);
  assert.equal(git(clone, "status", "--porcelain"), "");
});

test("a staged change of someone else's is exit 1 and is neither committed nor touched", (t) => {
  const { clone } = releasedOrigin(t);
  writeFileSync(join(clone, "local.md"), "mine\n");
  git(clone, "add", "local.md");
  const head = git(clone, "rev-parse", "HEAD");
  const r = run(clone, ...BETA);
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /^commit-back: the working tree is not clean \(local\.md\); nothing committed$/m,
  );
  assert.equal(git(clone, "rev-parse", "HEAD"), head);
  assert.equal(git(clone, "status", "--porcelain"), "A  local.md");
  assert.equal(
    readFileSync(join(clone, "release-notes/index.json"), "utf8"),
    EMPTY_INDEX,
  );
});

test("a local commit that is not on origin/main is exit 1 and nothing is committed or pushed", (t) => {
  const { origin, clone } = releasedOrigin(t);
  writeFileSync(join(clone, "local.md"), "mine\n");
  git(clone, "add", "local.md");
  git(clone, "commit", "-q", "-m", "local");
  const head = git(clone, "rev-parse", "HEAD");
  const before = git(origin, "rev-parse", "main");
  const r = run(clone, ...BETA);
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /^commit-back: HEAD [0-9a-f]{40} is not origin\/main [0-9a-f]{40}; nothing committed$/m,
  );
  assert.equal(git(clone, "rev-parse", "HEAD"), head);
  assert.equal(git(origin, "rev-parse", "main"), before);
});

test("a beta commits only its index entry", (t) => {
  const { clone } = releasedOrigin(t);
  const r = run(clone, ...BETA, "--no-push");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(
    git(clone, "log", "-1", "--format=%s"),
    "Beta v0.1.0-beta.1: index",
  );
  assert.equal(
    git(clone, "diff", "--name-status", "HEAD~1", "HEAD"),
    "M\trelease-notes/index.json",
  );
  assert.deepEqual(indexAt(clone, "HEAD"), {
    releases: [entry("beta", "v0.1.0-beta.1")],
  });
});

test("an index entry that conflicts with the indexed one is exit 1 and commits nothing", (t) => {
  const indexed = { ...entry("release", "v0.1.0"), date: "2026-11-01" };
  const { clone } = releasedOrigin(t, {
    "release-notes/index.json": `${JSON.stringify({ releases: [indexed] }, null, 2)}\n`,
  });
  const head = git(clone, "rev-parse", "HEAD");
  const r = run(clone, ...RELEASE, "--no-push");
  assert.equal(r.status, 1);
  assert.match(r.stderr, /index entry for v0\.1\.0 differs/);
  assert.match(r.stderr, /^commit-back: .*nothing committed/m);
  assert.equal(git(clone, "rev-parse", "HEAD"), head);
  assert.equal(git(clone, "status", "--porcelain"), "");
});

test("a fragment on main that fails the schema check is exit 1 and commits nothing", (t) => {
  const { clone } = releasedOrigin(t, {
    "changes/broken.md": "no front matter\n",
  });
  const head = git(clone, "rev-parse", "HEAD");
  const r = run(clone, ...RELEASE, "--no-push");
  assert.equal(r.status, 1);
  assert.match(r.stderr, /changes\/broken\.md/);
  assert.match(r.stderr, /^commit-back: .*nothing committed/m);
  assert.equal(git(clone, "rev-parse", "HEAD"), head);
  // The index entry, the notes archive and the fragment removal are undone.
  assert.equal(git(clone, "status", "--porcelain"), "");
});

test("a malformed index on main is exit 1 and commits nothing", (t) => {
  const { clone } = releasedOrigin(t, {
    "release-notes/index.json": '{ "releases": {} }\n',
  });
  const head = git(clone, "rev-parse", "HEAD");
  const r = run(clone, ...BETA, "--no-push");
  assert.equal(r.status, 1);
  assert.match(r.stderr, /release-notes\/index\.json/);
  assert.equal(git(clone, "rev-parse", "HEAD"), head);
});

test("pushes the commit to main on origin", (t) => {
  const { origin, clone } = releasedOrigin(t);
  const r = run(clone, ...BETA);
  assert.equal(r.status, 0, r.stderr);
  assert.equal(
    git(origin, "rev-parse", "main"),
    git(clone, "rev-parse", "HEAD"),
  );
  assert.equal(
    git(origin, "log", "-1", "--format=%s", "main"),
    "Beta v0.1.0-beta.1: index",
  );
});

test("a push rejected because main moved is retried once after a rebase onto it", (t) => {
  const { author, origin, clone } = releasedOrigin(t);
  // main moves on origin after the clone was made, before the script pushes.
  author.commit("meanwhile", { "docs/meanwhile.md": "x\n" });
  author.run("push", "-q", origin, "main");
  const r = run(clone, ...RELEASE);
  assert.equal(r.status, 0, r.stderr);
  assert.match(
    r.stdout,
    /^commit-back: the push to origin\/main failed; fetching, rebasing onto it and retrying once$/m,
  );
  assert.equal(
    git(origin, "log", "-2", "--format=%s", "main"),
    `${RELEASE_MESSAGE}\nmeanwhile`,
  );
  assert.equal(
    git(origin, "rev-parse", "main~1"),
    author.run("rev-parse", "HEAD"),
  );
});

test("a push that is still rejected after the rebase is exit 1 and points at the runbook", (t) => {
  const { origin, clone } = releasedOrigin(t);
  const hook = join(origin, "hooks", "pre-receive");
  writeFileSync(hook, "#!/bin/sh\necho 'rejected by the test' >&2\nexit 1\n");
  chmodSync(hook, 0o755);
  const before = git(origin, "rev-parse", "main");
  const r = run(clone, ...BETA);
  assert.equal(r.status, 1);
  assert.match(
    r.stdout,
    /^commit-back: the push to origin\/main failed; fetching, rebasing onto it and retrying once$/m,
  );
  assert.match(
    r.stderr,
    /^commit-back: .*failed again.*docs\/release-management\.md#if-the-commit-to-main-failed/m,
  );
  assert.equal(git(origin, "rev-parse", "main"), before);
});

test("a rebase that conflicts is aborted, exit 1 with the runbook, and nothing is pushed", (t) => {
  const { author, origin, clone } = releasedOrigin(t);
  // Meanwhile another commit-back indexed the beta on main.
  author.commit("Beta v0.1.0-beta.1: index", {
    "release-notes/index.json": `${JSON.stringify({ releases: [entry("beta", "v0.1.0-beta.1")] }, null, 2)}\n`,
  });
  author.run("push", "-q", origin, "main");
  const before = git(origin, "rev-parse", "main");
  const r = run(clone, ...RELEASE);
  assert.equal(r.status, 1);
  assert.match(
    r.stdout,
    /^commit-back: the push to origin\/main failed; fetching, rebasing onto it and retrying once$/m,
  );
  assert.match(
    r.stderr,
    /^commit-back: .*the commit is not on origin\/main.*docs\/release-management\.md#if-the-commit-to-main-failed$/m,
  );
  assert.equal(git(origin, "rev-parse", "main"), before);
  assert.equal(git(clone, "status", "--porcelain"), "");
  assert.ok(!existsSync(join(clone, ".git", "rebase-merge")));
  assert.ok(!existsSync(join(clone, ".git", "rebase-apply")));
});

test("a fragment that main gained meanwhile and that fails the schema stops the retried push", (t) => {
  const { author, origin, clone } = releasedOrigin(t);
  author.commit("broken", { "changes/broken.md": "no front matter\n" });
  author.run("push", "-q", origin, "main");
  const before = git(origin, "rev-parse", "main");
  const r = run(clone, ...BETA);
  assert.equal(r.status, 1);
  assert.match(
    r.stdout,
    /^commit-back: the push to origin\/main failed; fetching, rebasing onto it and retrying once$/m,
  );
  assert.match(r.stderr, /changes\/broken\.md/);
  assert.match(
    r.stderr,
    /^commit-back: .*not pushed.*if-the-commit-to-main-failed$/m,
  );
  assert.equal(git(origin, "rev-parse", "main"), before);
});

test("a stage that does not match the tag is exit 1 and commits nothing", (t) => {
  const { clone } = releasedOrigin(t);
  const head = git(clone, "rev-parse", "HEAD");
  const r = run(
    clone,
    "--tag",
    "v0.1.0",
    "--stage",
    "beta",
    "--version",
    "0.1.0",
    "--date",
    "2026-11-02",
    "--no-push",
  );
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /the new entry \(v0\.1\.0\): tag "v0\.1\.0" is not v0\.1\.0-beta\.N/,
  );
  assert.match(r.stderr, /^commit-back: .*nothing committed/m);
  assert.equal(git(clone, "rev-parse", "HEAD"), head);
  assert.equal(
    readFileSync(join(clone, "release-notes/index.json"), "utf8"),
    EMPTY_INDEX,
  );
});

test("a missing option is a usage error, exit 2", (t) => {
  const { clone } = releasedOrigin(t);
  const r = run(clone, "--tag", "v0.1.0", "--stage", "release");
  assert.equal(r.status, 2);
  assert.match(r.stderr, /^usage: commit-back/);
});

test("parseMain splits <remote>/<branch> at the first slash and refuses anything else", () => {
  assert.deepEqual(parseMain("origin/main"), {
    remote: "origin",
    branch: "main",
  });
  assert.deepEqual(parseMain("upstream/release/0.1"), {
    remote: "upstream",
    branch: "release/0.1",
  });
  for (const bad of ["main", "/main", "origin/"])
    assert.throws(
      () => parseMain(bad),
      new RegExp(`^Error: --main ${bad} is not <remote>/<branch>$`),
      bad,
    );
});

test("a --main that is not <remote>/<branch> is exit 1 before anything is committed", (t) => {
  const { clone } = releasedOrigin(t);
  const head = git(clone, "rev-parse", "HEAD");
  const r = run(clone, ...BETA, "--main", "main");
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /^commit-back: --main main is not <remote>\/<branch>; nothing committed$/m,
  );
  assert.equal(git(clone, "rev-parse", "HEAD"), head);
  assert.equal(git(clone, "status", "--porcelain"), "");
});

/** A clone with one local commit on top of origin/main, ready to push. */
const withLocalCommit = (t) => {
  const fixture = releasedOrigin(t);
  writeFileSync(join(fixture.clone, "release-notes", "note.md"), "x\n");
  git(fixture.clone, "add", ".");
  git(fixture.clone, "commit", "-q", "-m", "local");
  return fixture;
};
const ORIGIN_MAIN = { remote: "origin", branch: "main" };
const quiet = () => {};

test("pushWithRetry pushes once and never runs the check when the first push lands", (t) => {
  const { origin, clone } = withLocalCommit(t);
  let checks = 0;
  pushWithRetry(ORIGIN_MAIN, {
    check: () => checks++,
    git: { cwd: clone, env: GIT_ENV },
    log: quiet,
  });
  assert.equal(checks, 0);
  assert.equal(
    git(origin, "rev-parse", "main"),
    git(clone, "rev-parse", "HEAD"),
  );
});

test("pushWithRetry rebases onto the moved branch, runs the check on the rebased tree, then pushes", (t) => {
  const { author, origin, clone } = withLocalCommit(t);
  author.commit("meanwhile", { "docs/meanwhile.md": "x\n" });
  author.run("push", "-q", origin, "main");
  const logged = [];
  let checkedOn = null;
  pushWithRetry(ORIGIN_MAIN, {
    check: () => {
      checkedOn = git(clone, "rev-parse", "HEAD~1");
    },
    git: { cwd: clone, env: GIT_ENV },
    log: (line) => logged.push(line),
  });
  assert.equal(checkedOn, author.run("rev-parse", "HEAD"));
  assert.equal(
    git(origin, "log", "-2", "--format=%s", "main"),
    "local\nmeanwhile",
  );
  assert.deepEqual(logged, [
    "commit-back: the push to origin/main failed; fetching, rebasing onto it and retrying once",
  ]);
});

test("pushWithRetry stops when the check fails after the rebase, and pushes nothing", (t) => {
  const { author, origin, clone } = withLocalCommit(t);
  author.commit("meanwhile", { "docs/meanwhile.md": "x\n" });
  author.run("push", "-q", origin, "main");
  const before = git(origin, "rev-parse", "main");
  assert.throws(
    () =>
      pushWithRetry(ORIGIN_MAIN, {
        check: () => {
          throw new Error("changes/broken.md: no front matter");
        },
        git: { cwd: clone, env: GIT_ENV },
        log: quiet,
      }),
    /^Error: changes\/broken\.md: no front matter after the rebase onto origin\/main; the commit is not pushed, make it by hand: docs\/release-management\.md#if-the-commit-to-main-failed$/,
  );
  assert.equal(git(origin, "rev-parse", "main"), before);
});

test("nothingCommitted restores the tree and says nothing was committed", () => {
  let restored = false;
  assert.equal(
    nothingCommitted(new Error("index entry differs"), () => {
      restored = true;
    }),
    "index entry differs; nothing committed",
  );
  assert.ok(restored);
});

test("nothingCommitted keeps the original failure when restoring the tree fails too", () => {
  assert.equal(
    nothingCommitted(new Error("index entry differs"), () => {
      throw new Error("git reset -q --hard HEAD failed: index.lock exists");
    }),
    "index entry differs; nothing committed, but restoring the working tree failed (git reset -q --hard HEAD failed: index.lock exists): reset it by hand with git reset --hard HEAD and git clean -fd -- release-notes changes",
  );
});

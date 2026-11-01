import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { GIT_ENV, makeRepo } from "./lib/test-repo.mjs";

const script = join(
  dirname(fileURLToPath(import.meta.url)),
  "shipped-fragments.mjs",
);

const fragment = (line) =>
  `---\ntype: internal\ntickets: [mt08.aaaa]\ncomponents: [core]\n---\n${line}\n`;

/**
 * main with fragments a, b and c; release/0.1 cut from it, a and b deleted
 * on the branch (the final notes PR) and tagged v0.1.0; on main meanwhile b
 * rewritten under the same slug. Leaves main checked out.
 */
const releasedRepo = (t) => {
  const repo = makeRepo();
  t.after(() => repo.dispose());
  repo.commit("fragments", {
    "changes/README.md": "# guide\n",
    "changes/a.md": fragment("A."),
    "changes/b.md": fragment("B."),
    "changes/c.md": fragment("C."),
  });
  repo.run("switch", "-q", "-c", "release/0.1");
  repo.remove("changes/a.md");
  repo.remove("changes/b.md");
  repo.commit("final notes", { "release-notes/0.1.0.md": "# 0.1.0\n" });
  repo.tag("v0.1.0");
  repo.run("switch", "-q", "main");
  repo.commit("b reused", { "changes/b.md": fragment("Another B.") });
  return repo;
};

const blob = (repo, ref, path) => repo.run("rev-parse", `${ref}:${path}`);

const run = (repo, ...args) =>
  spawnSync(process.execPath, [script, ...args], {
    cwd: repo.dir,
    env: GIT_ENV,
    encoding: "utf8",
  });

test("lists the fragments deleted on the release branch with their shipped blobs, in path order", (t) => {
  const repo = releasedRepo(t);
  const r = run(repo, "--tag", "v0.1.0", "--main", "main");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stderr, "");
  assert.equal(
    r.stdout,
    `${blob(repo, "v0.1.0~1", "changes/a.md")} changes/a.md\n${blob(repo, "v0.1.0~1", "changes/b.md")} changes/b.md\n`,
  );
});

test("--delete removes a fragment whose content is the shipped blob and keeps a reused slug, naming it", (t) => {
  const repo = releasedRepo(t);
  const r = run(repo, "--tag", "v0.1.0", "--main", "main", "--delete");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(
    r.stdout,
    "removed changes/a.md\nkept changes/b.md: differs from the shipped fragment (slug reused?)\n",
  );
  assert.doesNotMatch(r.stdout + r.stderr, /c\.md/);
  assert.ok(!existsSync(join(repo.dir, "changes/a.md")));
  assert.ok(existsSync(join(repo.dir, "changes/b.md")));
  assert.ok(existsSync(join(repo.dir, "changes/c.md")));
  assert.equal(
    repo.run("diff", "--cached", "--name-status"),
    "D\tchanges/a.md",
  );
});

test("--delete reports a shipped fragment that is no longer on main as absent", (t) => {
  const repo = releasedRepo(t);
  repo.run("rm", "-q", "changes/a.md");
  repo.commit("a gone");
  const r = run(repo, "--tag", "v0.1.0", "--main", "main", "--delete");
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /^absent changes\/a\.md\n/);
});

test("a fragment added on the branch after the cut and deleted before the tag counts as shipped", (t) => {
  const repo = makeRepo();
  t.after(() => repo.dispose());
  repo.commit("start", { "changes/README.md": "# guide\n" });
  repo.run("switch", "-q", "-c", "release/0.1");
  repo.commit("cherry-picked fix", { "changes/fix.md": fragment("Fix.") });
  const shipped = blob(repo, "HEAD", "changes/fix.md");
  repo.remove("changes/fix.md");
  repo.commit("final notes");
  repo.tag("v0.1.0");
  repo.run("switch", "-q", "main");
  const r = run(repo, "--tag", "v0.1.0", "--main", "main");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout, `${shipped} changes/fix.md\n`);
});

test("only fragments count: README.md and nested paths under changes/ are not listed", (t) => {
  const repo = makeRepo();
  t.after(() => repo.dispose());
  repo.commit("start", {
    "changes/README.md": "# guide\n",
    "changes/sub/x.md": "x\n",
  });
  repo.run("switch", "-q", "-c", "release/0.1");
  repo.remove("changes/README.md");
  repo.remove("changes/sub/x.md");
  repo.commit("odd deletions");
  repo.tag("v0.1.0");
  repo.run("switch", "-q", "main");
  const r = run(repo, "--tag", "v0.1.0", "--main", "main");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout, "");
});

test("a fragment edited on the branch but not deleted did not ship", (t) => {
  const repo = makeRepo();
  t.after(() => repo.dispose());
  repo.commit("start", { "changes/a.md": fragment("A.") });
  repo.run("switch", "-q", "-c", "release/0.1");
  repo.commit("edit", { "changes/a.md": fragment("A, reworded.") });
  repo.tag("v0.1.0");
  repo.run("switch", "-q", "main");
  const r = run(repo, "--tag", "v0.1.0", "--main", "main");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout, "");
});

test("nothing deleted on the branch: the list and --delete print nothing", (t) => {
  const repo = makeRepo();
  t.after(() => repo.dispose());
  repo.commit("start", { "changes/a.md": fragment("A.") });
  repo.run("switch", "-q", "-c", "release/0.1");
  repo.commit("notes", { "release-notes/0.1.0.md": "# 0.1.0\n" });
  repo.tag("v0.1.0");
  repo.run("switch", "-q", "main");
  for (const extra of [[], ["--delete"]]) {
    const r = run(repo, "--tag", "v0.1.0", "--main", "main", ...extra);
    assert.equal(r.status, 0, r.stderr);
    assert.equal(r.stdout, "", extra.join(" "));
  }
  assert.ok(existsSync(join(repo.dir, "changes/a.md")));
});

test("--main defaults to origin/main", (t) => {
  const repo = releasedRepo(t);
  repo.run("update-ref", "refs/remotes/origin/main", "main");
  const r = run(repo, "--tag", "v0.1.0");
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, / changes\/a\.md\n.* changes\/b\.md\n$/);
});

test("--tag must be an existing release tag", (t) => {
  const repo = releasedRepo(t);
  repo.tag("v0.1.0-beta.1", "v0.1.0");
  for (const tag of ["v0.1.0-beta.1", "v0.2.0", "release"]) {
    const r = run(repo, "--tag", tag, "--main", "main");
    assert.equal(r.status, 1, tag);
    assert.equal(r.stdout, "", tag);
    assert.match(r.stderr, new RegExp(`^shipped-fragments: .*${tag}`), tag);
  }
});

test("--help prints the usage; a missing --tag is a usage error", (t) => {
  const repo = releasedRepo(t);
  const help = run(repo, "--help");
  assert.equal(help.status, 0);
  assert.match(help.stdout, /^usage: shipped-fragments/);
  for (const args of [[], ["--delete"], ["--tag", "v0.1.0", "x"]]) {
    const r = run(repo, ...args);
    assert.equal(r.status, 2, args.join(" "));
    assert.match(r.stderr, /^usage: shipped-fragments/, args.join(" "));
  }
});

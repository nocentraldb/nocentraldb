import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { GIT_ENV, makeRepo } from "./lib/test-repo.mjs";

const script = join(dirname(fileURLToPath(import.meta.url)), "cut.mjs");

const CARGO = `[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.package]
version = "0.1.0"
edition = "2024"
`;
const CRATE =
  '[package]\nname = "a"\nversion.workspace = true\nedition.workspace = true\n';
const PACKAGE = `${JSON.stringify({ name: "@x/p", version: "0.1.0", private: true }, null, 2)}\n`;

/** A repository on main with the manifests at 0.1.0 and a bare origin. */
const repoWithOrigin = (t) => {
  const repo = makeRepo();
  const origin = mkdtempSync(join(tmpdir(), "cut-origin-"));
  t.after(() => {
    repo.dispose();
    rmSync(origin, { recursive: true, force: true });
  });
  repo.commit("start", {
    "Cargo.toml": CARGO,
    "crates/a/Cargo.toml": CRATE,
    "crates/a/src/lib.rs": "",
    "packages/p/package.json": PACKAGE,
  });
  spawnSync("git", ["init", "-q", "--bare", origin], { env: GIT_ENV });
  repo.run("remote", "add", "origin", origin);
  repo.run("push", "-q", "origin", "main");
  return repo;
};

const run = (repo, args, env = { CUT_NO_LOCK: "1" }) =>
  spawnSync(process.execPath, [script, ...args], {
    cwd: repo.dir,
    env: { ...GIT_ENV, ...env },
    encoding: "utf8",
  });

const branch = (repo) => repo.run("rev-parse", "--abbrev-ref", "HEAD");
const head = (repo, ref) => repo.run("rev-parse", ref);
const onOrigin = (repo, ref) =>
  repo.run("ls-remote", "origin", `refs/heads/${ref}`);
const version = (repo, ref) =>
  /version = "([^"]+)"/.exec(repo.run("show", `${ref}:Cargo.toml`))[1];

test("the first cut of a minor creates release/X.Y with no commit, ends on main and prints the next steps", (t) => {
  const repo = repoWithOrigin(t);
  const main = head(repo, "main");
  const r = run(repo, ["0.1.0"]);
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stderr, "");
  assert.equal(head(repo, "release/0.1"), main);
  assert.equal(branch(repo), "main");
  assert.equal(repo.run("status", "--porcelain"), "");
  assert.equal(onOrigin(repo, "release/0.1"), "");
  assert.match(r.stdout, /cut: manifests already at 0\.1\.0, no commit\n/);
  assert.match(r.stdout, /\n {2}git push -u origin release\/0\.1\n/);
  assert.match(
    r.stdout,
    /node scripts\/release\/set-version\.mjs 0\.2\.0 && git switch -c release-0\.1-raise && git commit -am "Raise main to 0\.2\.0 after cutting release\/0\.1" && gh pr create --label skip-changes --title "Raise main to 0\.2\.0" --body "/,
  );
  assert.match(
    r.stdout,
    /git tag v0\.1\.0-beta\.1 release\/0\.1 && git push origin refs\/tags\/v0\.1\.0-beta\.1\n/,
  );
});

test("--from cuts from the given commit", (t) => {
  const repo = repoWithOrigin(t);
  const first = head(repo, "main");
  repo.commit("later");
  const r = run(repo, ["0.1.0", "--from", first]);
  assert.equal(r.status, 0, r.stderr);
  assert.equal(head(repo, "release/0.1"), first);
});

test("a minor whose manifests differ gets one commit setting the version", (t) => {
  const repo = repoWithOrigin(t);
  const main = head(repo, "main");
  const r = run(repo, ["0.2.0"]);
  assert.equal(r.status, 0, r.stderr);
  assert.equal(head(repo, "release/0.2~1"), main);
  assert.equal(
    repo.run("log", "-1", "--format=%s", "release/0.2"),
    "Set the version to 0.2.0 on release/0.2",
  );
  assert.equal(version(repo, "release/0.2"), "0.2.0");
  assert.match(
    repo.run("show", "release/0.2:packages/p/package.json"),
    /"version": "0\.2\.0"/,
  );
  assert.equal(version(repo, "main"), "0.1.0");
  assert.equal(branch(repo), "main");
});

test("a patch commits the version change on the existing release branch and ends on the branch it started from", (t) => {
  const repo = repoWithOrigin(t);
  assert.equal(run(repo, ["0.1.0", "--push"]).status, 0);
  repo.run("switch", "-q", "-c", "feature");
  const before = head(repo, "release/0.1");
  const r = run(repo, ["0.1.1", "--from", "release/0.1"]);
  assert.equal(r.status, 0, r.stderr);
  assert.equal(branch(repo), "feature");
  assert.equal(head(repo, "release/0.1~1"), before);
  assert.equal(
    repo.run("log", "-1", "--format=%s", "release/0.1"),
    "Set the version to 0.1.1 on release/0.1",
  );
  assert.equal(version(repo, "release/0.1"), "0.1.1");
  assert.match(r.stdout, /main needs no raise for a patch/);
  assert.doesNotMatch(r.stdout, /gh pr create/);
  assert.match(r.stdout, /git tag v0\.1\.1-beta\.1 release\/0\.1/);
  assert.match(r.stdout, /git push -u origin release\/0\.1\n/);
});

test("--push pushes the branch to origin", (t) => {
  const repo = repoWithOrigin(t);
  const r = run(repo, ["0.1.0", "--push"]);
  assert.equal(r.status, 0, r.stderr);
  assert.match(onOrigin(repo, "release/0.1"), /refs\/heads\/release\/0\.1$/);
  assert.doesNotMatch(r.stdout, /\n {2}git push -u origin/);
  assert.equal(branch(repo), "main");
});

test("refuses a minor whose release branch exists locally or on origin", (t) => {
  const repo = repoWithOrigin(t);
  repo.run("branch", "release/0.1");
  const local = run(repo, ["0.1.0"]);
  assert.equal(local.status, 1);
  assert.equal(
    local.stderr,
    "cut: release/0.1 already exists locally; a patch of it is X.Y.Z with Z > 0\n",
  );
  repo.run("push", "-q", "origin", "release/0.1");
  repo.run("branch", "-D", "release/0.1");
  const remote = run(repo, ["0.1.0"]);
  assert.equal(remote.status, 1);
  assert.match(remote.stderr, /^cut: release\/0\.1 already exists on origin/);
  assert.equal(branch(repo), "main");
});

test("refuses a patch whose release branch does not exist, or whose --from is not its head", (t) => {
  const repo = repoWithOrigin(t);
  const missing = run(repo, ["0.1.1"]);
  assert.equal(missing.status, 1);
  assert.match(missing.stderr, /^cut: release\/0\.1 does not exist locally/);
  assert.equal(run(repo, ["0.1.0"]).status, 0);
  repo.commit("on main");
  const elsewhere = run(repo, ["0.1.1", "--from", "main"]);
  assert.equal(elsewhere.status, 1);
  assert.match(
    elsewhere.stderr,
    /^cut: a patch is cut on the head of release\/0\.1, not on main/,
  );
});

test("refuses a patch when the local release branch differs from origin's", (t) => {
  const repo = repoWithOrigin(t);
  assert.equal(run(repo, ["0.1.0", "--push"]).status, 0);
  repo.run("switch", "-q", "release/0.1");
  repo.commit("local only");
  repo.run("switch", "-q", "main");
  const r = run(repo, ["0.1.1"]);
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /^cut: release\/0\.1 differs from origin\/release\/0\.1/,
  );
});

test("a version that is not a plain X.Y.Z is a usage error", (t) => {
  const repo = repoWithOrigin(t);
  for (const args of [["0.1.0-beta.1"], ["v0.1.0"], [], ["0.1.0", "--x"]]) {
    const r = run(repo, args);
    assert.equal(r.status, 2, args.join(" "));
    assert.match(r.stderr, /^usage: cut/);
  }
  assert.equal(repo.run("branch", "--list", "release/*"), "");
});

test("refuses a dirty working tree, an untracked file included, and creates nothing", (t) => {
  const repo = repoWithOrigin(t);
  repo.write("notes.txt", "scratch\n");
  const r = run(repo, ["0.1.0"]);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /^cut: the working tree is not clean/);
  assert.equal(repo.run("branch", "--list", "release/*"), "");
});

test("refuses a detached HEAD", (t) => {
  const repo = repoWithOrigin(t);
  repo.run("switch", "-q", "--detach", "main");
  const r = run(repo, ["0.1.0"]);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /^cut: HEAD is detached/);
});

test("a failure after the switch still ends on the branch it started from, with a clean tree", (t) => {
  const repo = repoWithOrigin(t);
  assert.equal(run(repo, ["0.1.0"]).status, 0);
  repo.run("switch", "-q", "-c", "feature");
  const before = head(repo, "release/0.1");
  // set-version refreshes Cargo.lock with cargo; this cargo always fails.
  const bin = mkdtempSync(join(tmpdir(), "cut-bin-"));
  t.after(() => rmSync(bin, { recursive: true, force: true }));
  writeFileSync(
    join(bin, "cargo"),
    "#!/bin/sh\necho cargo broke >&2\nexit 1\n",
  );
  chmodSync(join(bin, "cargo"), 0o755);
  const r = run(repo, ["0.1.1"], { PATH: `${bin}:${process.env.PATH}` });
  assert.equal(r.status, 1);
  assert.match(r.stderr, /cut: node .*set-version\.mjs 0\.1\.1 failed/);
  assert.equal(branch(repo), "feature");
  assert.equal(repo.run("status", "--porcelain"), "");
  assert.equal(head(repo, "release/0.1"), before);
  assert.equal(
    readFileSync(join(repo.dir, "Cargo.toml"), "utf8"),
    CARGO,
    "the start branch's manifests are untouched",
  );
});

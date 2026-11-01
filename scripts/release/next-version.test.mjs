import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { GIT_ENV, makeRepo } from "./lib/test-repo.mjs";

const script = join(
  dirname(fileURLToPath(import.meta.url)),
  "next-version.mjs",
);

const cargo = (version) =>
  `[workspace]\nmembers = []\n\n[workspace.package]\nversion = "${version}"\n`;
const index = (...releases) => `${JSON.stringify({ releases }, null, 2)}\n`;
const entry = (version, stage = "release") => ({
  version,
  stage,
  tag: stage === "release" ? `v${version}` : `v${version}-beta.1`,
  date: "2026-01-01",
  path: `release-notes/${version}.md`,
});

const FRAGMENT = {
  feature:
    "---\ntype: feature\ntickets: [mt08.aaaa]\ncomponents: [core]\n---\n## Problem\na\n## Use\nb\n",
  breaking:
    "---\ntype: breaking\ntickets: [mt08.aaaa]\ncomponents: [core]\n---\n## Problem\na\n## Use\nb\n## Migration\nc\n",
  fix: "---\ntype: fix\ntickets: [mt08.aaaa]\nintroduced: cycle\ncomponents: [core]\n---\n## Symptom\na\n## When\nb\n## Cause\nc\n## Fix\nd\n",
  internal:
    "---\ntype: internal\ntickets: [mt08.aaaa]\ncomponents: [core]\n---\nTidied the build.\n",
};

/** A repository whose first commit holds Cargo.toml at `version`. */
const repoFor = (t, { version = "0.1.0", files = {} } = {}) => {
  const repo = makeRepo();
  t.after(() => repo.dispose());
  repo.commit("start", { "Cargo.toml": cargo(version), ...files });
  return repo;
};

const run = (repo, ...args) =>
  spawnSync(process.execPath, [script, "--json", ...args], {
    cwd: repo.dir,
    env: GIT_ENV,
    encoding: "utf8",
  });

/** The parsed --json answer of a successful run. */
const next = (repo, ...args) => {
  const r = run(repo, ...args);
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stderr, "");
  return JSON.parse(r.stdout);
};

test("the first alpha: one feature fragment, no tags", (t) => {
  const repo = repoFor(t);
  repo.commit("add x", { "changes/add-x.md": FRAGMENT.feature });
  assert.deepEqual(next(repo), {
    skip: false,
    target: "0.1.0",
    n: 1,
    version: "0.1.0-alpha.1",
    tag: "v0.1.0-alpha.1",
    previousAlpha: null,
    lastRelease: null,
    fragments: ["changes/add-x.md"],
  });
});

test("the next alpha lists only the fragments added since the previous one", (t) => {
  const repo = repoFor(t);
  repo.commit("add x", { "changes/add-x.md": FRAGMENT.feature });
  repo.tag("v0.1.0-alpha.1");
  repo.commit("add y", { "changes/add-y.md": FRAGMENT.fix });
  const n = next(repo);
  assert.equal(n.version, "0.1.0-alpha.2");
  assert.equal(n.tag, "v0.1.0-alpha.2");
  assert.equal(n.previousAlpha, "v0.1.0-alpha.1");
  assert.deepEqual(n.fragments, ["changes/add-y.md"]);
});

test("skips when only an internal fragment was added since the previous alpha", (t) => {
  const repo = repoFor(t);
  repo.commit("add x", { "changes/add-x.md": FRAGMENT.feature });
  repo.tag("v0.1.0-alpha.1");
  repo.commit("add y", { "changes/add-y.md": FRAGMENT.fix });
  repo.tag("v0.1.0-alpha.2");
  repo.commit("tidy", { "changes/tidy.md": FRAGMENT.internal });
  assert.deepEqual(next(repo), {
    skip: true,
    reason: "no fragment other than internal since v0.1.0-alpha.2",
    previousAlpha: "v0.1.0-alpha.2",
    fragments: [],
  });
});

test("skips when no fragment was added since the previous alpha", (t) => {
  const repo = repoFor(t);
  repo.commit("add x", { "changes/add-x.md": FRAGMENT.feature });
  repo.tag("v0.1.0-alpha.2");
  repo.commit("docs only", { "docs/a.md": "a\n" });
  const n = next(repo);
  assert.equal(n.skip, true);
  assert.match(n.reason, /since v0\.1\.0-alpha\.2$/);
});

test("skips when no fragment exists at all", (t) => {
  const repo = repoFor(t, { files: { "changes/README.md": "# guide\n" } });
  assert.deepEqual(next(repo), {
    skip: true,
    reason: "no fragment other than internal since the start of the history",
    previousAlpha: null,
    fragments: [],
  });
});

test("a breaking fragment raises the target; only it is listed since the previous alpha of the old target", (t) => {
  const repo = repoFor(t, {
    version: "1.3.0",
    files: { "release-notes/index.json": index(entry("1.2.0")) },
  });
  repo.commit("add x", { "changes/add-x.md": FRAGMENT.feature });
  repo.tag("v1.3.0-alpha.1");
  repo.commit("break y", { "changes/break-y.md": FRAGMENT.breaking });
  const n = next(repo);
  assert.equal(n.target, "2.0.0");
  assert.equal(n.version, "2.0.0-alpha.1");
  assert.equal(n.lastRelease, "1.2.0");
  assert.equal(n.previousAlpha, "v1.3.0-alpha.1");
  assert.deepEqual(n.fragments, ["changes/break-y.md"]);
});

test("the manifest version wins when it is higher than the bump", (t) => {
  const repo = repoFor(t, {
    version: "0.2.0",
    files: { "release-notes/index.json": index(entry("0.1.0")) },
  });
  repo.commit("fix y", { "changes/fix-y.md": FRAGMENT.fix });
  const n = next(repo);
  assert.equal(n.lastRelease, "0.1.0");
  assert.equal(n.version, "0.2.0-alpha.1");
});

test("the bump wins when it is higher than the manifest version", (t) => {
  const repo = repoFor(t, {
    version: "0.1.0",
    files: { "release-notes/index.json": index(entry("0.1.0")) },
  });
  repo.commit("fix y", { "changes/fix-y.md": FRAGMENT.fix });
  assert.equal(next(repo).version, "0.1.1-alpha.1");
});

test("the last release is the highest release entry of the index, never a beta entry", (t) => {
  const repo = repoFor(t, {
    version: "0.1.0",
    files: {
      "release-notes/index.json": index(
        entry("0.2.0"),
        entry("0.10.0"),
        entry("0.11.0", "beta"),
      ),
    },
  });
  repo.commit("fix y", { "changes/fix-y.md": FRAGMENT.fix });
  const n = next(repo);
  assert.equal(n.lastRelease, "0.10.0");
  assert.equal(n.version, "0.10.1-alpha.1");
});

test("the last release is read from the index, not from a release tag", (t) => {
  const repo = repoFor(t, { version: "0.1.0" });
  repo.tag("v0.1.0");
  repo.commit("add x", { "changes/add-x.md": FRAGMENT.feature });
  const n = next(repo);
  assert.equal(n.lastRelease, null);
  assert.equal(n.target, "0.1.0");
  assert.equal(n.version, "0.1.0-alpha.1");
});

test("the previous alpha follows ancestry; n counts every alpha tag of the target", (t) => {
  const repo = repoFor(t);
  repo.commit("add x", { "changes/add-x.md": FRAGMENT.feature });
  repo.tag("v0.1.0-alpha.1");
  repo.run("switch", "-q", "-c", "side");
  repo.commit("side", { "changes/side.md": FRAGMENT.feature });
  repo.tag("v0.1.0-alpha.5");
  repo.run("switch", "-q", "main");
  repo.commit("add y", { "changes/add-y.md": FRAGMENT.fix });
  const n = next(repo);
  assert.equal(n.previousAlpha, "v0.1.0-alpha.1");
  assert.equal(n.n, 6);
  assert.equal(n.version, "0.1.0-alpha.6");
  assert.deepEqual(n.fragments, ["changes/add-y.md"]);
});

test("alpha tags of another target do not count towards n", (t) => {
  const repo = repoFor(t, { version: "0.2.0" });
  repo.tag("v0.1.0-alpha.7");
  repo.tag("v0.2.0-beta.3");
  repo.commit("add x", { "changes/add-x.md": FRAGMENT.feature });
  assert.equal(next(repo).version, "0.2.0-alpha.1");
});

test("a tag that only looks like an alpha of the target does not count towards n", (t) => {
  const repo = repoFor(t);
  repo.tag("v0.1.0-alpha.9-rc");
  repo.commit("add x", { "changes/add-x.md": FRAGMENT.feature });
  assert.equal(next(repo).version, "0.1.0-alpha.1");
});

test("the bump counts every present fragment, also those of earlier alphas", (t) => {
  const repo = repoFor(t, {
    files: { "release-notes/index.json": index(entry("0.1.0")) },
  });
  repo.commit("add x", { "changes/add-x.md": FRAGMENT.feature });
  repo.tag("v0.2.0-alpha.1");
  repo.commit("fix y", { "changes/fix-y.md": FRAGMENT.fix });
  const n = next(repo);
  assert.equal(n.version, "0.2.0-alpha.2");
  assert.deepEqual(n.fragments, ["changes/fix-y.md"]);
});

test("--head computes against that commit, fragments and manifest included", (t) => {
  const repo = repoFor(t);
  const first = repo.commit("add x", { "changes/add-x.md": FRAGMENT.feature });
  repo.commit("later", {
    "Cargo.toml": cargo("0.3.0"),
    "changes/add-y.md": FRAGMENT.fix,
  });
  const n = next(repo, "--head", first);
  assert.equal(n.version, "0.1.0-alpha.1");
  assert.deepEqual(n.fragments, ["changes/add-x.md"]);
});

test("without --json it prints the tag, or the reason for skipping", (t) => {
  const repo = repoFor(t);
  const plain = (r) => {
    assert.equal(r.status, 0, r.stderr);
    return r.stdout;
  };
  const runPlain = () =>
    spawnSync(process.execPath, [script], {
      cwd: repo.dir,
      env: GIT_ENV,
      encoding: "utf8",
    });
  assert.equal(
    plain(runPlain()),
    "skip: no fragment other than internal since the start of the history\n",
  );
  repo.commit("add x", { "changes/add-x.md": FRAGMENT.feature });
  assert.equal(plain(runPlain()), "v0.1.0-alpha.1\n");
});

test("an invalid fragment fails the run and is named, even one added before the previous alpha", (t) => {
  const repo = repoFor(t);
  repo.commit("bad", { "changes/bad.md": "no front matter\n" });
  repo.tag("v0.1.0-alpha.1");
  repo.commit("add x", { "changes/add-x.md": FRAGMENT.feature });
  const r = run(repo);
  assert.equal(r.status, 1);
  assert.equal(r.stdout, "");
  assert.match(r.stderr, /^next-version: changes\/bad\.md: no front matter/);
});

test("a missing or invalid workspace version fails the run, naming Cargo.toml", (t) => {
  const repo = repoFor(t, { version: "0.1.0-alpha.1" });
  repo.commit("add x", { "changes/add-x.md": FRAGMENT.feature });
  let r = run(repo);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /^next-version: Cargo\.toml: .*0\.1\.0-alpha\.1/);
  repo.commit("no version", { "Cargo.toml": "[workspace]\nmembers = []\n" });
  r = run(repo);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /^next-version: Cargo\.toml: /);
  repo.remove("Cargo.toml");
  repo.commit("no manifest");
  r = run(repo);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /^next-version: Cargo\.toml: missing at HEAD$/m);
});

test("an index that does not parse, or holds a bad version, fails the run, naming it", (t) => {
  const repo = repoFor(t, {
    files: { "release-notes/index.json": "{ nope\n" },
  });
  repo.commit("add x", { "changes/add-x.md": FRAGMENT.feature });
  let r = run(repo);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /^next-version: release-notes\/index\.json: /);
  repo.commit("bad version", {
    "release-notes/index.json": index(entry("1.0")),
  });
  r = run(repo);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /^next-version: release-notes\/index\.json: .*"1\.0"/);
  repo.commit("no list", { "release-notes/index.json": "{}\n" });
  r = run(repo);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /^next-version: release-notes\/index\.json: /);
});

test("--index reads the index from another path", (t) => {
  const repo = repoFor(t, {
    files: { "other/index.json": index(entry("0.1.0")) },
  });
  repo.commit("fix y", { "changes/fix-y.md": FRAGMENT.fix });
  assert.equal(next(repo, "--index", "other/index.json").lastRelease, "0.1.0");
});

test("a head that does not exist fails the run, naming it", (t) => {
  const repo = repoFor(t);
  const r = run(repo, "--head", "nope");
  assert.equal(r.status, 1);
  assert.match(r.stderr, /^next-version: .*nope/);
});

test("--help prints the usage; an unknown argument is a usage error", (t) => {
  const repo = repoFor(t);
  const help = spawnSync(process.execPath, [script, "--help"], {
    cwd: repo.dir,
    encoding: "utf8",
  });
  assert.equal(help.status, 0);
  assert.match(help.stdout, /^usage: next-version/);
  for (const args of [["--nope"], ["--head"]]) {
    const r = spawnSync(process.execPath, [script, ...args], {
      cwd: repo.dir,
      encoding: "utf8",
    });
    assert.equal(r.status, 2, args.join(" "));
    assert.match(r.stderr, /^usage: next-version/);
  }
});

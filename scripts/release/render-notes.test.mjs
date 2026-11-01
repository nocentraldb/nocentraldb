import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { GIT_ENV, makeRepo } from "./lib/test-repo.mjs";

const script = join(
  dirname(fileURLToPath(import.meta.url)),
  "render-notes.mjs",
);

const FEATURE =
  "---\ntype: feature\ntickets: [mt08.aaaa, mt08.bbbb]\ncomponents: [core, sdk-ts]\n---\n\n## Problem\n\nNo way to x.\n\n## Use\n\nCall x().\n";
const FIX =
  "---\ntype: fix\ntickets: [mt08.cccc]\nintroduced: cycle\ncomponents: [relay]\n---\n## Symptom\ns\n## When\nw\n## Cause\nc\n## Fix\nf\n";
const INTERNAL =
  "---\ntype: internal\ntickets: [mt08.dddd]\ncomponents: [core]\n---\nTidied the build.\n";

const repoFor = (t) => {
  const repo = makeRepo();
  t.after(() => repo.dispose());
  repo.commit("start", { "changes/README.md": "# guide\n" });
  return repo;
};

const run = (repo, ...args) =>
  spawnSync(process.execPath, [script, ...args], {
    cwd: repo.dir,
    env: GIT_ENV,
    encoding: "utf8",
  });
const alpha = (repo, since, ...rest) =>
  run(repo, "--alpha", "--since", since, "--repo", "acme/db", ...rest);

test("renders the fragments added since the tag in slug order, with demoted headings, the metadata line and the previous alpha", (t) => {
  const repo = repoFor(t);
  repo.commit("old", { "changes/old.md": INTERNAL });
  repo.tag("v0.1.0-alpha.1");
  repo.commit("z first", { "changes/zeta-fix.md": FIX });
  repo.commit("a second", { "changes/alpha-x.md": FEATURE });
  const r = alpha(repo, "v0.1.0-alpha.1");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stderr, "");
  assert.equal(
    r.stdout,
    [
      "## alpha-x",
      "",
      "*feature* · components: core, sdk-ts · tickets: mt08.aaaa, mt08.bbbb",
      "",
      "### Problem",
      "",
      "No way to x.",
      "",
      "### Use",
      "",
      "Call x().",
      "",
      "## zeta-fix",
      "",
      "*fix* · components: relay · tickets: mt08.cccc",
      "",
      "### Symptom",
      "s",
      "### When",
      "w",
      "### Cause",
      "c",
      "### Fix",
      "f",
      "",
      "Previous alpha: [v0.1.0-alpha.1](https://github.com/acme/db/releases/tag/v0.1.0-alpha.1)",
      "",
    ].join("\n"),
  );
});

test("--since none renders every fragment present and ends with First alpha.", (t) => {
  const repo = repoFor(t);
  repo.commit("one", { "changes/b-tidy.md": INTERNAL });
  repo.commit("two", { "changes/a-fix.md": FIX });
  const r = alpha(repo, "none");
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /^## a-fix\n[\s\S]*\n## b-tidy\n/);
  assert.match(
    r.stdout,
    /\*internal\* · components: core · tickets: mt08\.dddd\n\nTidied the build\.\n/,
  );
  assert.ok(r.stdout.endsWith("\n\nFirst alpha.\n"), r.stdout);
  assert.doesNotMatch(r.stdout, /Previous alpha/);
});

test("the previous-alpha link is built from --repo", (t) => {
  const repo = repoFor(t);
  repo.tag("v2.0.0-alpha.3");
  repo.commit("one", { "changes/a-fix.md": FIX });
  const r = run(
    repo,
    "--alpha",
    "--since",
    "v2.0.0-alpha.3",
    "--repo",
    "other/name",
  );
  assert.equal(r.status, 0, r.stderr);
  assert.ok(
    r.stdout.endsWith(
      "\n\nPrevious alpha: [v2.0.0-alpha.3](https://github.com/other/name/releases/tag/v2.0.0-alpha.3)\n",
    ),
    r.stdout,
  );
});

test("a level-2 heading inside a fenced code block is not demoted", (t) => {
  const repo = repoFor(t);
  const fenced = FEATURE.replace(
    "Call x().\n",
    "Call x().\n\n```md\n## not a section\n```\n",
  );
  repo.commit("one", { "changes/a-x.md": fenced });
  const r = alpha(repo, "none");
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /```md\n## not a section\n```/);
});

test("--head renders against that commit", (t) => {
  const repo = repoFor(t);
  const first = repo.commit("one", { "changes/a-fix.md": FIX });
  repo.commit("two", { "changes/b-x.md": FEATURE });
  const r = alpha(repo, "none", "--head", first);
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /^## a-fix\n/);
  assert.doesNotMatch(r.stdout, /b-x/);
});

test("an invalid fragment fails the run, naming the file, and prints no notes", (t) => {
  const repo = repoFor(t);
  repo.commit("one", { "changes/a-fix.md": FIX });
  repo.commit("two", { "changes/bad.md": "---\ntype: nope\n---\n" });
  const r = alpha(repo, "none");
  assert.equal(r.status, 1);
  assert.equal(r.stdout, "");
  assert.match(r.stderr, /^render-notes: changes\/bad\.md: type must be/m);
});

test("--since must be an alpha tag that exists, or none", (t) => {
  const repo = repoFor(t);
  repo.tag("v0.1.0-beta.1");
  for (const since of ["v0.1.0-beta.1", "v0.1.0-alpha.1"]) {
    const r = alpha(repo, since);
    assert.equal(r.status, 1, since);
    assert.equal(r.stdout, "");
    assert.match(r.stderr, new RegExp(`^render-notes: .*${since}`), since);
  }
});

test("--beta and --release are not implemented in this ticket", (t) => {
  const repo = repoFor(t);
  for (const mode of ["--beta", "--release"]) {
    const r = run(repo, mode, "0.1.0");
    assert.equal(r.status, 2, mode);
    assert.match(r.stderr, /not implemented in this ticket/);
  }
});

test("--help prints the usage; missing or malformed arguments are usage errors", (t) => {
  const repo = repoFor(t);
  const help = run(repo, "--help");
  assert.equal(help.status, 0);
  assert.match(help.stdout, /^usage: render-notes/);
  for (const args of [
    ["--since", "none", "--repo", "acme/db"],
    ["--alpha", "--repo", "acme/db"],
    ["--alpha", "--since", "none"],
    ["--alpha", "--since", "none", "--repo", "acme"],
    ["--alpha", "--since", "none", "--repo", "acme/db", "--nope"],
  ]) {
    const r = run(repo, ...args);
    assert.equal(r.status, 2, args.join(" "));
    assert.match(r.stderr, /^usage: render-notes/, args.join(" "));
  }
});

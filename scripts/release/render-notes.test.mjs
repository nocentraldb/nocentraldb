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
  for (const line of r.stderr.trimEnd().split("\n"))
    assert.match(
      line,
      /^render-notes: changes\/bad\.md: /,
      "not a clean failure",
    );
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

const NOTES =
  "# NoCentralDB 0.1.0\n\nThe first release.\n\n## New features\n\nX.\n";

/** A repository with the curated notes of 0.1.0 and one fragment, tagged v0.1.0-beta.1. */
const betaRepo = (t) => {
  const repo = repoFor(t);
  repo.commit("notes", {
    "release-notes/0.1.0.md": NOTES,
    "changes/early.md": FEATURE,
  });
  repo.tag("v0.1.0-beta.1");
  return repo;
};

test("--beta --since none prints the curated file exactly as it is at the head", (t) => {
  const repo = betaRepo(t);
  const r = run(repo, "--beta", "0.1.0", "--since", "none");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stderr, "");
  assert.equal(r.stdout, NOTES);
});

test("--beta --since <previous beta> adds the fragments added since that tag, unfiltered, one level down", (t) => {
  const repo = betaRepo(t);
  repo.commit("cycle fix", { "changes/zeta-fix.md": FIX });
  repo.commit("tidy", { "changes/b-tidy.md": INTERNAL });
  const r = run(repo, "--beta", "0.1.0", "--since", "v0.1.0-beta.1");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stderr, "");
  assert.equal(
    r.stdout,
    [
      NOTES.trimEnd(),
      "",
      "## Changes since v0.1.0-beta.1",
      "",
      "### b-tidy",
      "",
      "*internal* · components: core · tickets: mt08.dddd",
      "",
      "Tidied the build.",
      "",
      "### zeta-fix",
      "",
      "*fix* · components: relay · tickets: mt08.cccc",
      "",
      "#### Symptom",
      "s",
      "#### When",
      "w",
      "#### Cause",
      "c",
      "#### Fix",
      "f",
      "",
    ].join("\n"),
  );
  assert.doesNotMatch(r.stdout, /early/, "a fragment of beta.1 is listed");
});

test("--beta --since <previous beta> with no fragment added says so", (t) => {
  const repo = betaRepo(t);
  repo.commit("notes edited", {
    "release-notes/0.1.0.md": `${NOTES}\nMore.\n`,
  });
  const r = run(repo, "--beta", "0.1.0", "--since", "v0.1.0-beta.1");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(
    r.stdout,
    `${NOTES}\nMore.\n\n## Changes since v0.1.0-beta.1\n\nNo change fragment was added since v0.1.0-beta.1.\n`,
  );
});

test("--beta reads the file and the fragments at --head, never from the working tree", (t) => {
  const repo = betaRepo(t);
  const head = repo.commit("fix", { "changes/zeta-fix.md": FIX });
  repo.commit("later", { "changes/later.md": INTERNAL });
  repo.write("release-notes/0.1.0.md", "uncommitted\n");
  const r = run(
    repo,
    "--beta",
    "0.1.0",
    "--since",
    "v0.1.0-beta.1",
    "--head",
    head,
  );
  assert.equal(r.status, 0, r.stderr);
  assert.ok(r.stdout.startsWith(NOTES.trimEnd()), r.stdout);
  assert.match(r.stdout, /### zeta-fix/);
  assert.doesNotMatch(r.stdout, /later|uncommitted/);
});

test("--release prints only the curated file, whatever fragments exist", (t) => {
  const repo = betaRepo(t);
  repo.commit("fix", { "changes/zeta-fix.md": FIX });
  const r = run(repo, "--release", "0.1.0");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stderr, "");
  assert.equal(r.stdout, NOTES);
});

test("--beta and --release fail naming the notes file when it does not exist at the head", (t) => {
  const repo = repoFor(t);
  repo.commit("one", { "changes/a-fix.md": FIX });
  for (const args of [
    ["--beta", "0.2.0", "--since", "none"],
    ["--release", "0.2.0"],
  ]) {
    const r = run(repo, ...args);
    assert.equal(r.status, 1, args.join(" "));
    assert.equal(r.stdout, "", args.join(" "));
    assert.equal(
      r.stderr,
      "render-notes: release-notes/0.2.0.md does not exist at HEAD\n",
      args.join(" "),
    );
  }
});

test("--beta --since must be an existing beta tag of the same version, or none", (t) => {
  const repo = betaRepo(t);
  repo.tag("v0.1.0-alpha.1");
  repo.tag("v0.2.0-beta.1");
  for (const since of [
    "v0.1.0-alpha.1",
    "v0.2.0-beta.1",
    "v0.1.0-beta.9",
    "v0.1.0",
  ]) {
    const r = run(repo, "--beta", "0.1.0", "--since", since);
    assert.equal(r.status, 1, since);
    assert.equal(r.stdout, "", since);
    assert.match(r.stderr, new RegExp(`^render-notes: .*${since}`), since);
  }
});

test("--beta with an invalid fragment added since the tag fails naming it", (t) => {
  const repo = betaRepo(t);
  repo.commit("bad", { "changes/bad.md": "---\ntype: nope\n---\n" });
  const r = run(repo, "--beta", "0.1.0", "--since", "v0.1.0-beta.1");
  assert.equal(r.status, 1);
  assert.equal(r.stdout, "");
  assert.match(r.stderr, /^render-notes: changes\/bad\.md: type must be/m);
});

test("--help prints the usage; missing or malformed arguments are usage errors", (t) => {
  const repo = repoFor(t);
  const help = run(repo, "--help");
  assert.equal(help.status, 0);
  assert.match(help.stdout, /^usage: render-notes/);
  for (const args of [
    ["--beta", "0.1.0"],
    ["--beta", "0.1", "--since", "none"],
    ["--beta", "v0.1.0", "--since", "none"],
    ["--release", "0.1.0-beta.1"],
    ["--release", "0.1.0", "--since", "none"],
    ["--release", "0.1.0", "--beta", "0.1.0", "--since", "none"],
    ["--alpha", "--since", "none", "--repo", "acme/db", "--release", "0.1.0"],
    ["--beta", "0.1.0", "--since", "none", "--repo", "acme/db"],
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

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { GIT_ENV, makeRepo } from "./lib/test-repo.mjs";

const script = join(dirname(fileURLToPath(import.meta.url)), "draft-notes.mjs");

const front = (type, ticket, extra = "") =>
  `---\ntype: ${type}\ntickets: [${ticket}]\n${extra}components: [core]\n---\n`;
const BREAKING = `${front("breaking", "mt08.brk1")}\n## Problem\n\nOpen took a path.\n\n## Use\n\nPass a URL.\n\n## Migration\n\n\`\`\`rust\nopen(url)\n\`\`\`\n`;
const FEATURE = `${front("feature", "mt08.fea1")}\n## Problem\n\nNo way to watch a table.\n\n## Use\n\nCall watch().\n`;
const DEPRECATION = `${front("deprecation", "mt08.dep1")}\n## Deprecated\n\nold()\n\n## Replacement\n\nnew()\n\n## Removed in\n\n2.0.0\n`;
const FIX = `${front("fix", "mt08.fix1", "introduced: 0.1.0\n")}\n## Symptom\n\nSync stalls.\n\n## When\n\nOffline.\n\n## Cause\n\nA lock.\n\n## Fix\n\nNo lock.\n`;
const CYCLE = (n) =>
  `${front("fix", `mt08.cyc${n}`, "introduced: cycle\n")}\n## Symptom\n\nCrash ${n}.\n\n## When\n\nAlways.\n\n## Cause\n\nNew code.\n\n## Fix\n\nFixed.\n`;
const SECURITY = `${front("security", "mt08.sec1")}\n## Description\n\nA token leaked into logs.\n\n## Advisory\n\npending\n`;
const INTERNAL = `${front("internal", "mt08.int1")}Tidied the build.\n`;
const INDEX = (releases) => `${JSON.stringify({ releases }, null, 2)}\n`;
const NO_RELEASES = INDEX([]);

const repoWith = (t, files) => {
  const repo = makeRepo();
  t.after(() => repo.dispose());
  repo.commit("start", {
    "changes/README.md": "# guide\n",
    "release-notes/index.json": NO_RELEASES,
    ...files,
  });
  return repo;
};

const run = (repo, ...args) =>
  spawnSync(process.execPath, [script, ...args], {
    cwd: repo.dir,
    env: GIT_ENV,
    encoding: "utf8",
  });

const notes = (repo, version = "0.2.0") =>
  readFileSync(join(repo.dir, `release-notes/${version}.md`), "utf8");

const EVERY_TYPE = {
  "changes/z-breaking.md": BREAKING,
  "changes/y-feature.md": FEATURE,
  "changes/x-deprecation.md": DEPRECATION,
  "changes/w-fix.md": FIX,
  "changes/v-cycle.md": CYCLE(1),
  "changes/u-security.md": SECURITY,
  "changes/t-internal.md": INTERNAL,
};

test("writes the sections in the order of the curated notes, each entry titled, with its tickets and bold labels", (t) => {
  const repo = repoWith(t, EVERY_TYPE);
  const r = run(repo, "0.2.0");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stderr, "");
  assert.equal(
    notes(repo),
    [
      "# 0.2.0",
      "",
      "<!-- Summary: two or three sentences on what this release is for. -->",
      "",
      "## Breaking changes",
      "",
      "### Open took a path.",
      "",
      "<!-- z-breaking · tickets: mt08.brk1 -->",
      "",
      "**Problem.** Open took a path.",
      "",
      "**Use.** Pass a URL.",
      "",
      "**Migration.**",
      "",
      "```rust",
      "open(url)",
      "```",
      "",
      "## New features",
      "",
      "### No way to watch a table.",
      "",
      "<!-- y-feature · tickets: mt08.fea1 -->",
      "",
      "**Problem.** No way to watch a table.",
      "",
      "**Use.** Call watch().",
      "",
      "## Deprecations",
      "",
      "### old()",
      "",
      "<!-- x-deprecation · tickets: mt08.dep1 -->",
      "",
      "**Deprecated.** old()",
      "",
      "**Replacement.** new()",
      "",
      "**Removed in.** 2.0.0",
      "",
      "## Fixes",
      "",
      "### Sync stalls.",
      "",
      "<!-- w-fix · tickets: mt08.fix1 -->",
      "",
      "**Symptom.** Sync stalls.",
      "",
      "**When.** Offline.",
      "",
      "**Cause.** A lock.",
      "",
      "**Fix.** No lock.",
      "",
      "## Security",
      "",
      "### A token leaked into logs.",
      "",
      "<!-- u-security · tickets: mt08.sec1 -->",
      "",
      "**Description.** A token leaked into logs.",
      "",
      "**Advisory.** pending",
      "",
      "## Known issues",
      "",
      "<!-- Bugs introduced in this cycle that are known and not fixed, from the board; each with a workaround if there is one. -->",
      "",
      "<!-- Filtered (cycle fixes, not shown to users): v-cycle -->",
      "<!-- Filtered (internal, not shown to users): t-internal -->",
      "",
    ].join("\n"),
  );
  assert.match(r.stdout, /release-notes\/0\.2\.0\.md/);
});

test("writes only the sections that have entries, Known issues always", (t) => {
  const repo = repoWith(t, {
    "changes/a-feature.md": FEATURE,
    "changes/b-cycle.md": CYCLE(1),
    "changes/c-cycle.md": CYCLE(2),
  });
  const r = run(repo, "0.2.0");
  assert.equal(r.status, 0, r.stderr);
  const text = notes(repo);
  const headings = text.split("\n").filter((l) => l.startsWith("## "));
  assert.deepEqual(headings, ["## New features", "## Known issues"]);
  assert.doesNotMatch(text, /Crash/);
  assert.match(
    text,
    /<!-- Filtered \(cycle fixes, not shown to users\): b-cycle, c-cycle -->\n$/,
  );
  assert.doesNotMatch(text, /internal, not shown/);
});

test("the cycle fix and the internal fragment are absent from the body", (t) => {
  const repo = repoWith(t, EVERY_TYPE);
  assert.equal(run(repo, "0.2.0").status, 0);
  const text = notes(repo);
  assert.doesNotMatch(text, /Crash 1|Tidied the build|mt08\.cyc1|mt08\.int1/);
});

test("an entry whose first section is empty is titled by its slug", (t) => {
  const repo = repoWith(t, {
    "changes/empty-problem.md": `${front("feature", "mt08.fea2")}\n## Problem\n\n## Use\n\nCall it.\n`,
  });
  assert.equal(run(repo, "0.2.0").status, 0);
  assert.match(notes(repo), /\n### empty-problem\n/);
});

test("--out writes elsewhere", (t) => {
  const repo = repoWith(t, { "changes/a-feature.md": FEATURE });
  const r = run(repo, "0.2.0", "--out", "draft.md");
  assert.equal(r.status, 0, r.stderr);
  assert.match(readFileSync(join(repo.dir, "draft.md"), "utf8"), /^# 0\.2\.0/);
  assert.equal(existsSync(join(repo.dir, "release-notes/0.2.0.md")), false);
});

test("an existing out file is exit 1 and left as it was; --force overwrites it", (t) => {
  const repo = repoWith(t, {
    "changes/a-feature.md": FEATURE,
    "release-notes/0.2.0.md": "# edited by hand\n",
  });
  const refused = run(repo, "0.2.0");
  assert.equal(refused.status, 1);
  assert.match(
    refused.stderr,
    /^draft-notes: release-notes\/0\.2\.0\.md exists; pass --force to overwrite it\n$/,
  );
  assert.equal(notes(repo), "# edited by hand\n");
  const forced = run(repo, "0.2.0", "--force");
  assert.equal(forced.status, 0, forced.stderr);
  assert.match(notes(repo), /^# 0\.2\.0\n/);
});

test("an invalid fragment is exit 1 naming the file, and nothing is written", (t) => {
  const repo = repoWith(t, {
    "changes/bad.md": `${front("feature", "mt08.fea1")}\n## Problem\n\nx\n`,
  });
  const r = run(repo, "0.2.0");
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /^draft-notes: changes\/bad\.md: missing section ## Use/,
  );
  assert.equal(existsSync(join(repo.dir, "release-notes/0.2.0.md")), false);
});

test("a version that is not X.Y.Z is a usage error", (t) => {
  const repo = repoWith(t, {});
  for (const args of [["0.2.0-beta.1"], [], ["0.2.0", "--bogus"]]) {
    const r = run(repo, ...args);
    assert.equal(r.status, 2, args.join(" "));
    assert.match(r.stderr, /^usage: draft-notes/);
  }
});

/**
 * main: `early` (a cycle fix) added, then the merge base M; release/0.1 cut
 * from M and tagged v0.1.0 there; then `late` (a cycle fix) added on main.
 * The index on main lists the release v0.1.0.
 */
const releasedRepo = (t) => {
  const repo = repoWith(t, {});
  repo.commit("early fix", { "changes/early.md": CYCLE(1) });
  repo.commit("M");
  repo.run("switch", "-q", "-c", "release/0.1");
  repo.commit("notes", { "release-notes/0.1.0.md": "# 0.1.0\n" });
  repo.tag("v0.1.0");
  repo.run("switch", "-q", "main");
  repo.commit("late fix and index", {
    "changes/late.md": CYCLE(2),
    "release-notes/index.json": INDEX([
      {
        version: "0.1.0",
        stage: "release",
        tag: "v0.1.0",
        date: "2026-01-01",
        path: "release-notes/0.1.0.md",
      },
    ]),
  });
  return repo;
};

const STALE =
  "draft-notes: changes/early.md says introduced: cycle but was added before v0.1.0 shipped — if the bug is in 0.1.0, change it to introduced: 0.1.0\n";

test("warns for a cycle fix added before the previous release's merge base, not for one added after (auto from the index)", (t) => {
  const repo = releasedRepo(t);
  const r = run(repo, "0.2.0");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stderr, STALE);
  const text = notes(repo);
  assert.match(
    text,
    /<!-- Filtered \(cycle fixes, not shown to users\): early, late -->\n<!-- Check introduced \(added before v0\.1\.0 shipped\): early -->\n$/,
  );
});

test("--previous names the release to check against", (t) => {
  const repo = releasedRepo(t);
  const r = run(repo, "0.2.0", "--previous", "v0.1.0");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stderr, STALE);
});

test("no release in the index means no stale-cycle check", (t) => {
  const repo = repoWith(t, { "changes/early.md": CYCLE(1) });
  repo.commit("later");
  const r = run(repo, "0.2.0");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stderr, "");
  assert.doesNotMatch(notes(repo), /Check introduced/);
});

test("--previous that is not a release tag, or a tag that does not exist, is exit 1 naming it", (t) => {
  const repo = releasedRepo(t);
  const missing = run(repo, "0.2.0", "--previous", "v0.0.9");
  assert.equal(missing.status, 1);
  assert.match(
    missing.stderr,
    /^draft-notes: v0\.0\.9 does not exist as a tag/,
  );
  const beta = run(repo, "0.2.0", "--previous", "v0.1.0-beta.1");
  assert.equal(beta.status, 2);
  assert.equal(existsSync(join(repo.dir, "release-notes/0.2.0.md")), false);
});

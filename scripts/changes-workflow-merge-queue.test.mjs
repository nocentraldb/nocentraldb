// Runs the steps of .github/workflows/changes.yml against a fixture laid out
// as GitHub's merge queue lays out a merge group (NOC-511). The queue builds
// a branch gh-readonly-queue/main/pr-<number>-<sha> whose commit squashes
// the pull request onto main as it stands, with any pull requests ahead of
// it in the queue; the `merge_group` run checks that commit out. Every
// comparison must start at its first parent, as on a pull request's merge
// ref (NOC-505): a change main gained is in the baseline, while the pull
// request's own changes, its skip-changes label and the Test-change trailers
// of its own commits still count. The squash commit's message keeps none of
// those trailers, so the PR step fetches the pull request's head.
//
// `gh` is replaced by a script on PATH that answers the one API call the PR
// step makes with the labels a test gives; it runs the step's --jq filter
// through jq. The PR's head is published under refs/pull/<number>/head of a
// remote `origin` that is the fixture repository itself.
import assert from "node:assert/strict";
import { chmodSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  FEATURE,
  MAIN_BREAKS,
  packageRepo,
  runStep,
  step,
} from "./lib/changes-fixture.mjs";

const prStep = step((s) => s.id === "pr", "merge-group PR");
const baselineStep = step((s) => s.id === "base-worktree", "baseline");
const apiStep = step((s) => s.id === "api", "API report");
const fragmentsStep = step(
  (s) => String(s.run).includes("check-fragments.mjs"),
  "required-fragment",
);
const testChangesStep = step(
  (s) => String(s.run).includes("check-test-changes.mjs"),
  "test-change",
);

const NUMBER = 7;
const REPOSITORY = "nocentraldb/nocentraldb";
const TEST = "packages/sdk/test/a.test.ts";
const TWO_TESTS = `it("keeps", () => {\n  expect(1).toBe(1);\n});\n\nit("goes", () => {\n  expect(2).toBe(2);\n});\n`;
const ONE_TEST = `it("keeps", () => {\n  expect(1).toBe(1);\n});\n`;

/**
 * The merge queue's commit for pull request NOC-1 (#7): main at `base` when
 * the PR branched, the PR's `commits` ({ message, files } each), main's newer
 * commit (`main` files), and the PR squashed onto main's tip, checked out
 * detached. The PR's head is published as refs/pull/7/head on `origin`.
 */
const mergeGroup = (t, { commits, main }) => {
  const { repo, commitWith, report } = packageRepo(t);
  const base = commitWith("base", { [TEST]: TWO_TESTS });
  repo.run("checkout", "-q", "-b", "pr");
  let head = base;
  for (const { message, files } of commits) head = commitWith(message, files);
  repo.run("checkout", "-q", "main");
  const mainTip = commitWith("main moves on", main);
  repo.run("checkout", "-q", "--detach", mainTip);
  try {
    repo.run("merge", "--squash", "-q", "pr");
  } catch {
    // Two edits of the API report conflict textually; the gate regenerates it.
  }
  report();
  repo.run("add", "-A");
  repo.run("commit", "-q", "-m", "NOC-1: the pull request (#7)");
  const groupHead = repo.run("rev-parse", "HEAD");
  repo.run("update-ref", `refs/pull/${NUMBER}/head`, head);
  repo.run("remote", "add", "origin", repo.dir);
  return { repo, base, head, mainTip, groupHead };
};

/** A directory with a fake `gh` that answers GET pulls/7 with `labels`. */
const fakeGh = (t, labels) => {
  const bin = mkdtempSync(join(tmpdir(), "changes-gh-"));
  t.after(() => rmSync(bin, { recursive: true, force: true }));
  const pull = JSON.stringify({ labels: labels.map((name) => ({ name })) });
  writeFileSync(
    join(bin, "gh"),
    `#!/bin/bash
[ "$1 $2 $3" = "api repos/${REPOSITORY}/pulls/${NUMBER} --jq" ] || { echo "unexpected: gh $*" >&2; exit 1; }
[ -n "$GH_TOKEN" ] || { echo "no GH_TOKEN" >&2; exit 1; }
printf '%s' '${pull}' | jq "$4"
`,
  );
  chmodSync(join(bin, "gh"), 0o755);
  return { PATH: `${bin}:${process.env.PATH}`, GITHUB_REPOSITORY: REPOSITORY };
};

/** The expressions of the steps' env in a merge_group run. */
const expressions = (fixture, outputs = {}) => ({
  "${{ github.event_name }}": "merge_group",
  "${{ github.token }}": "a-token",
  "${{ github.event.merge_group.head_ref }}": `refs/heads/gh-readonly-queue/main/pr-${NUMBER}-${fixture.mainTip}`,
  "${{ github.event.merge_group.head_sha }}": fixture.groupHead,
  "${{ github.event.pull_request.head.sha }}": "",
  "${{ (contains(github.event.pull_request.labels.*.name, 'skip-changes') || steps.pr.outputs.skip == 'true') && '--skip' || '' }}":
    outputs.skip === "true" ? "--skip" : "",
  "${{ (steps.semver.outcome == 'failure' || steps.api.outputs.breaking == 'true') && '--require-breaking' || '' }}":
    outputs.breaking === "true" ? "--require-breaking" : "",
  "${{ github.event.pull_request.base.sha || 'HEAD^1' }}": "HEAD^1",
  "${{ github.event.pull_request.head.sha || 'HEAD' }}": "HEAD",
  "${{ github.event.pull_request.head.sha || steps.pr.outputs.head }}":
    outputs.head ?? "",
});

/** Runs the PR, baseline, API, fragment and test-change steps in order. */
const runQueue = (t, fixture, labels = []) => {
  const { repo } = fixture;
  const gh = fakeGh(t, labels);
  const pr = runStep(t, repo, prStep, expressions(fixture), gh);
  assert.equal(pr.status, 0, pr.stderr);
  const baseline = runStep(t, repo, baselineStep, expressions(fixture));
  assert.equal(baseline.status, 0, baseline.stderr);
  const api = runStep(t, repo, apiStep, expressions(fixture));
  assert.equal(api.status, 0, api.stderr + api.stdout);
  const outputs = { ...pr.outputs, breaking: api.outputs.breaking };
  return {
    pr: pr.outputs,
    baselineCommit: repo.run(
      "-C",
      join(baseline.temp, "baseline"),
      "rev-parse",
      "HEAD",
    ),
    breaking: api.outputs.breaking,
    fragments: runStep(t, repo, fragmentsStep, expressions(fixture, outputs)),
    testChanges: runStep(
      t,
      repo,
      testChangesStep,
      expressions(fixture, outputs),
    ),
  };
};

test("the PR step finds the pull request's head and its label from the queue's branch", (t) => {
  const fixture = mergeGroup(t, {
    commits: [{ message: "NOC-1: docs", files: { "docs/a.md": "a\n" } }],
    main: MAIN_BREAKS,
  });
  const found = runQueue(t, fixture, ["skip-changes"]);
  assert.deepEqual(found.pr, { head: fixture.head, skip: "true" });
  assert.equal(runQueue(t, fixture, ["other"]).pr.skip, "false");
});

test("main's breaking change ahead of the PR is in the baseline, and the PR passes", (t) => {
  const fixture = mergeGroup(t, {
    commits: [
      {
        message: "NOC-1: c",
        files: {
          "packages/sdk/src/c.ts": "export const unused = 1;\n",
          "changes/pr-adds-c.md": FEATURE,
        },
      },
    ],
    main: MAIN_BREAKS,
  });
  const found = runQueue(t, fixture);
  assert.equal(found.baselineCommit, fixture.mainTip);
  assert.equal(found.breaking, "false");
  assert.equal(found.fragments.status, 0, found.fragments.stderr);
  assert.equal(found.testChanges.status, 0, found.testChanges.stderr);
});

test("the PR's own breaking change fails without a breaking fragment", (t) => {
  const fixture = mergeGroup(t, {
    commits: [
      {
        message: "NOC-1: rename",
        files: {
          "packages/sdk/src/a.ts":
            "export const renamed = (): string => 'sdk';\n",
          "changes/pr-renames-name.md": FEATURE,
        },
      },
    ],
    main: MAIN_BREAKS,
  });
  const found = runQueue(t, fixture, ["skip-changes"]);
  assert.equal(found.breaking, "true");
  assert.equal(found.fragments.status, 1);
  assert.match(
    found.fragments.stderr,
    /a breaking change was detected and no breaking fragment was added/,
  );
});

test("a product change without a fragment fails, unless the PR carries skip-changes", (t) => {
  const fixture = mergeGroup(t, {
    commits: [
      {
        message: "NOC-1: c",
        files: { "packages/sdk/src/c.ts": "export const unused = 1;\n" },
      },
    ],
    main: MAIN_BREAKS,
  });
  const without = runQueue(t, fixture);
  assert.equal(without.fragments.status, 1);
  assert.match(
    without.fragments.stderr,
    /packages\/sdk\/src\/c\.ts ships to a user/,
  );
  assert.doesNotMatch(without.fragments.stderr, /b\.ts/);
  const labelled = runQueue(t, fixture, ["skip-changes"]);
  assert.equal(labelled.fragments.status, 0, labelled.fragments.stderr);
});

test("a removed test passes on a Test-change trailer in one of the PR's commits, and fails without", (t) => {
  const removes = (message) => [
    { message, files: { [TEST]: ONE_TEST } },
    { message: "NOC-1: docs", files: { "docs/a.md": "a\n" } },
  ];
  const given = runQueue(
    t,
    mergeGroup(t, {
      commits: removes(
        "NOC-1: drop goes\n\nTest-change: goes duplicated keeps since NOC-7",
      ),
      main: MAIN_BREAKS,
    }),
  ).testChanges;
  assert.equal(given.status, 0, given.stderr);
  assert.match(given.stdout, /Test-change: goes duplicated keeps since NOC-7/);

  const missing = runQueue(
    t,
    mergeGroup(t, { commits: removes("NOC-1: drop goes"), main: MAIN_BREAKS }),
  ).testChanges;
  assert.equal(missing.status, 1);
  assert.match(missing.stderr, /removes the test `goes`/);
});

test("the baseline step fails when the checkout is not the queue's commit", (t) => {
  const fixture = mergeGroup(t, {
    commits: [{ message: "NOC-1: docs", files: { "docs/a.md": "a\n" } }],
    main: MAIN_BREAKS,
  });
  fixture.repo.run("checkout", "-q", "--detach", fixture.mainTip);
  const baseline = runStep(t, fixture.repo, baselineStep, expressions(fixture));
  assert.notEqual(baseline.status, 0);
});

test("the PR step fails on a branch that names no pull request", (t) => {
  const fixture = mergeGroup(t, {
    commits: [{ message: "NOC-1: docs", files: { "docs/a.md": "a\n" } }],
    main: MAIN_BREAKS,
  });
  const values = {
    ...expressions(fixture),
    "${{ github.event.merge_group.head_ref }}": "refs/heads/main",
  };
  const pr = runStep(t, fixture.repo, prStep, values, fakeGh(t, []));
  assert.notEqual(pr.status, 0);
});

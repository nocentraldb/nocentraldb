// Runs the steps of .github/workflows/changes.yml that pick the baseline and
// the fragment range against a fixture repository laid out as GitHub lays out
// a pull request run (NOC-505). actions/checkout checks out
// refs/pull/N/merge: a merge commit whose first parent is main's tip when the
// job runs, and whose second parent is the PR's head. Main may have moved
// since the PR's last push, so main's tip there is newer than the event's
// base.sha. The baseline of both API checks and the fragment range must come
// from that merge commit, so that a breaking change main gained in between is
// in the baseline and not counted against the PR, while the PR's own
// breaking change still is.
//
// The steps' `run` scripts are taken from the workflow and run by bash as
// GitHub runs them (-e, -o pipefail), with the event's values in their env.
// The TypeScript API report runs for real on a small fixture package; the
// Rust API check (cargo-semver-checks) cannot run here, so its baseline
// worktree is checked instead: it must be the merge commit's first parent.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";
import { GIT_ENV, makeRepo } from "./release/lib/test-repo.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const steps = parse(
  readFileSync(join(root, ".github/workflows/changes.yml"), "utf8"),
).jobs.changes.steps;
const step = (pred, what) => {
  const found = steps.find(pred);
  assert.ok(found, `changes.yml has no ${what} step`);
  return found;
};
const baselineStep = step((s) => s.id === "base-worktree", "baseline");
const apiStep = step((s) => s.id === "api", "API report");
const fragmentsStep = step(
  (s) => String(s.run).includes("check-fragments.mjs"),
  "required-fragment",
);

const FEATURE =
  "---\ntype: feature\ntickets: [mt08.1qaz]\ncomponents: [core]\n---\n## Problem\na\n## Use\nb\n";
const BREAKING =
  "---\ntype: breaking\ntickets: [mt08.1qaz]\ncomponents: [core]\n---\n## Problem\na\n## Use\nb\n## Migration\nc\n";

const A = "export const name = (): string => 'sdk';\n";
const B =
  "export const other = (): number => 1;\nexport const more = (): number => 2;\n";

const writeReport = (repo) => {
  const result = spawnSync(
    process.execPath,
    [join(root, "scripts/release/api-report.mjs"), "--write"],
    { cwd: repo.dir, env: GIT_ENV, encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
};

/**
 * A repository at a GitHub merge ref: main at `base` when the PR was pushed,
 * the PR's commit (`pr` files), then main's newer commit (`main` files), and
 * the merge commit checked out detached, its first parent main's new tip.
 * The API report is regenerated in each commit, as the gate requires.
 */
const mergeRef = (t, { pr, main }) => {
  const repo = makeRepo();
  t.after(() => repo.dispose());
  repo.write(
    "tsconfig.base.json",
    readFileSync(join(root, "tsconfig.base.json"), "utf8"),
  );
  repo.write(
    "packages/sdk/tsconfig.json",
    `${JSON.stringify({ extends: "../../tsconfig.base.json", include: ["src"] })}\n`,
  );
  repo.write(
    "packages/sdk/src/index.ts",
    'export * from "./a.ts";\nexport * from "./b.ts";\n',
  );
  repo.write("packages/sdk/src/a.ts", A);
  repo.write("packages/sdk/src/b.ts", B);
  // The workflow runs `node scripts/release/...` from the repository root.
  symlinkSync(join(root, "scripts"), join(repo.dir, "scripts"));
  writeFileSync(join(repo.dir, ".git/info/exclude"), "/scripts\n");
  const commitWith = (message, files) => {
    for (const [file, text] of Object.entries(files)) repo.write(file, text);
    writeReport(repo);
    return repo.commit(message);
  };
  const base = commitWith("base", {});
  repo.run("checkout", "-q", "-b", "pr");
  const head = commitWith("pr", pr);
  repo.run("checkout", "-q", "main");
  const mainTip = commitWith("main moves on", main);
  repo.run("checkout", "-q", "--detach", mainTip);
  spawnSync("git", ["merge", "--no-ff", "--no-commit", "-q", "pr"], {
    cwd: repo.dir,
    env: GIT_ENV,
  });
  // GitHub merges the report textually; regenerating it gives the same text
  // and resolves a conflict between two edits of it.
  writeReport(repo);
  repo.run("add", "-A");
  repo.run("commit", "-q", "-m", "merge pr into main");
  return { repo, base, head, mainTip };
};

/** Runs one step's script as GitHub does; returns its status and outputs. */
const runStep = (t, repo, theStep, values) => {
  const temp = mkdtempSync(join(tmpdir(), "changes-runner-"));
  t.after(() => rmSync(temp, { recursive: true, force: true }));
  const output = join(temp, "output");
  writeFileSync(output, "");
  const env = { ...GIT_ENV, RUNNER_TEMP: temp, GITHUB_OUTPUT: output };
  for (const [name, expression] of Object.entries(theStep.env ?? {})) {
    assert.ok(expression in values, `unknown expression ${expression}`);
    env[name] = values[expression];
  }
  const result = spawnSync(
    "bash",
    ["--noprofile", "--norc", "-eo", "pipefail", "-c", theStep.run],
    { cwd: repo.dir, env, encoding: "utf8" },
  );
  const outputs = Object.fromEntries(
    readFileSync(output, "utf8")
      .split("\n")
      .filter(Boolean)
      .map((line) => line.split("=", 2)),
  );
  return { ...result, outputs, temp };
};

/** The expressions of the steps' env, as the event and earlier steps set them. */
const expressions = ({ base, head }, breaking = "false") => ({
  "${{ github.event.pull_request.base.sha }}": base,
  "${{ github.event.pull_request.head.sha }}": head,
  "${{ contains(github.event.pull_request.labels.*.name, 'skip-changes') && '--skip' || '' }}":
    "",
  "${{ (steps.semver.outcome == 'failure' || steps.api.outputs.breaking == 'true') && '--require-breaking' || '' }}":
    breaking === "true" ? "--require-breaking" : "",
});

/** Runs the baseline, API and fragment steps; returns what each found. */
const runChanges = (t, fixture) => {
  const { repo } = fixture;
  const baseline = runStep(t, repo, baselineStep, expressions(fixture));
  assert.equal(baseline.status, 0, baseline.stderr);
  const worktree = join(baseline.temp, "baseline");
  const api = runStep(t, repo, apiStep, expressions(fixture));
  assert.equal(api.status, 0, api.stderr + api.stdout);
  const fragments = runStep(
    t,
    repo,
    fragmentsStep,
    expressions(fixture, api.outputs.breaking),
  );
  return {
    baselineCommit: repo.run("-C", worktree, "rev-parse", "HEAD"),
    breaking: api.outputs.breaking,
    fragments,
  };
};

// Main's breaking change: it removes `other`, with its own breaking fragment.
const MAIN_BREAKS = {
  "packages/sdk/src/b.ts": "export const more = (): number => 2;\n",
  "changes/main-drops-other.md": BREAKING,
};

test("main's breaking change after the PR's push is in the baseline, and the PR passes", (t) => {
  const fixture = mergeRef(t, {
    pr: {
      "packages/sdk/src/c.ts": "export const unused = 1;\n",
      "changes/pr-adds-c.md": FEATURE,
    },
    main: MAIN_BREAKS,
  });
  const found = runChanges(t, fixture);
  assert.equal(found.breaking, "false");
  assert.equal(found.fragments.status, 0, found.fragments.stderr);
  assert.equal(found.baselineCommit, fixture.mainTip);
});

test("the PR's own breaking change still fails without a breaking fragment, while main moved on", (t) => {
  const fixture = mergeRef(t, {
    pr: {
      "packages/sdk/src/a.ts": "export const renamed = (): string => 'sdk';\n",
      "changes/pr-renames-name.md": FEATURE,
    },
    main: MAIN_BREAKS,
  });
  const found = runChanges(t, fixture);
  assert.equal(found.baselineCommit, fixture.mainTip);
  assert.equal(found.breaking, "true");
  assert.equal(found.fragments.status, 1);
  assert.match(
    found.fragments.stderr,
    /a breaking change was detected and no breaking fragment was added/,
  );
});

test("a change the PR makes outside the allowlist still needs its fragment", (t) => {
  const fixture = mergeRef(t, {
    pr: { "packages/sdk/src/c.ts": "export const unused = 1;\n" },
    main: MAIN_BREAKS,
  });
  const found = runChanges(t, fixture);
  assert.equal(found.fragments.status, 1);
  assert.match(
    found.fragments.stderr,
    /packages\/sdk\/src\/c\.ts ships to a user/,
  );
  assert.doesNotMatch(found.fragments.stderr, /b\.ts/);
});

test("the baseline step fails when the checkout is not the PR's merge ref", (t) => {
  const fixture = mergeRef(t, {
    pr: { "changes/pr-adds-c.md": FEATURE },
    main: MAIN_BREAKS,
  });
  // A checkout of main's tip: its first parent is main's previous commit.
  fixture.repo.run("checkout", "-q", "--detach", fixture.mainTip);
  const baseline = runStep(t, fixture.repo, baselineStep, expressions(fixture));
  assert.notEqual(baseline.status, 0);
});

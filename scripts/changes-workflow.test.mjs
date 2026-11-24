// Asserts the invariants of .github/workflows/changes.yml, the `changes`
// check (epic mt08.q8rt §12): it runs on every pull request event that can
// change its answer, labeled and unlabeled included, so adding or removing
// the skip-changes label re-runs it; it reads only; it reports one job named
// `changes`; its actions are pinned by SHA; it fetches the full history; it
// passes the merge ref's first parent and the merge ref (HEAD^1 and HEAD)
// and the label-derived --skip to check-fragments; superseded runs are
// cancelled and the job is bounded. Breaking-change detection (epic §14.2):
// cargo-semver-checks against HEAD^1, the main the PR is merged into
// (NOC-505), allowed to fail so the fragment step still runs, and the
// TypeScript API report against the same HEAD^1; either one's finding turns
// into --require-breaking for check-fragments, which --skip never waives.
// scripts/changes-workflow-merge-ref.test.mjs runs these steps against a
// fixture merge ref.
// cargo-semver-checks checks only the publishable crates the base also has
// (scripts/release/semver-packages.mjs), and does not run when there are
// none: a crate new in the pull request has no baseline (NOC-253).
// The test-change check (NOC-508) runs last, with the PR's base and head,
// even when an earlier step failed, and its failure fails the job.
// In GitHub's merge queue (NOC-511) the same job runs on merge_group: the
// checkout is the queue's commit, the PR squashed onto main (or the PRs ahead
// of it), and every comparison starts at its first parent, HEAD^1, as on the
// merge ref. A step finds the PR from the queue's branch name, fetches its
// head and reads its skip-changes label (pull-requests: read); the
// test-change check runs on HEAD^1..HEAD and reads the trailers from the PR's
// own commits. scripts/changes-workflow-merge-queue.test.mjs runs these steps
// against a fixture queue commit.
//
// The workflow is parsed with the `yaml` package and checked by structure.
// Every check is also run against mutants of the file and must reject each
// one (scripts/lib/workflow-checks.mjs).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseDocument } from "yaml";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const load = () => {
  const doc = parseDocument(
    readFileSync(join(root, ".github/workflows/changes.yml"), "utf8"),
  );
  assert.deepEqual(doc.errors, [], "changes.yml does not parse");
  return { doc };
};
const clone = (ctx) => ({ doc: ctx.doc.clone() });

const workflow = (ctx) => ctx.doc.toJS();
const STEPS = ["jobs", "changes", "steps"];
const steps = (ctx) => workflow(ctx).jobs?.changes?.steps ?? [];

/** The index of the first step that satisfies `pred`. */
const at = (ctx, pred) => {
  const index = steps(ctx).findIndex(pred);
  assert.notEqual(index, -1, "no step matches");
  return index;
};
const checkoutStep = (ctx) =>
  at(ctx, (step) => step.uses?.startsWith("actions/checkout@"));
const fragmentsStep = (ctx) =>
  at(ctx, (step) => String(step.run).includes("check-fragments.mjs"));
const semverStep = (ctx) => at(ctx, (step) => step.id === "semver");
const apiStep = (ctx) => at(ctx, (step) => step.id === "api");
const baselineStep = (ctx) => at(ctx, (step) => step.id === "base-worktree");
const packagesStep = (ctx) => at(ctx, (step) => step.id === "packages");
const semverNoticeStep = (ctx) =>
  at(ctx, (step) => step.id === "semver-failed");
const prStep = (ctx) => at(ctx, (step) => step.id === "pr");
const testChangesStep = (ctx) =>
  at(ctx, (step) => String(step.run).includes("check-test-changes.mjs"));
const toolchainStep = (ctx) =>
  at(ctx, (step) => step.run === "rustup toolchain install --no-self-update");

/** A GitHub Actions expression, `${{ … }}`, as the workflow spells it. */
const gh = (expression) => `\${{ ${expression} }}`;

const TYPES = ["opened", "synchronize", "reopened", "labeled", "unlabeled"];
const ON = {
  pull_request: { types: TYPES },
  merge_group: { types: ["checks_requested"] },
};
const FRAGMENTS_ENV = {
  SKIP: gh(
    "(contains(github.event.pull_request.labels.*.name, 'skip-changes') || steps.pr.outputs.skip == 'true') && '--skip' || ''",
  ),
  BREAKING: gh(
    "(steps.semver.outcome == 'failure' || steps.api.outputs.breaking == 'true') && '--require-breaking' || ''",
  ),
};
const FRAGMENTS_RUN =
  "node scripts/release/check-fragments.mjs --base HEAD^1 --head HEAD $SKIP $BREAKING";
const SEMVER_ACTION = "obi1kenobi/cargo-semver-checks-action@";
// The merge ref's first parent, the main the PR is merged into: neither the
// event's base tip nor the merge base, which miss what main gained since the
// PR's push (NOC-505).
// In the merge queue the checkout is the queue's commit itself, whose first
// parent is main with the PRs ahead of it (NOC-511).
const BASELINE_ENV = {
  EVENT: gh("github.event_name"),
  HEAD: gh("github.event.pull_request.head.sha"),
  GROUP_HEAD: gh("github.event.merge_group.head_sha"),
};
const BASELINE_RUN = `if [ "$EVENT" = merge_group ]; then
  test "$(git rev-parse HEAD)" = "$GROUP_HEAD"
else
  test "$(git rev-parse HEAD^2)" = "$HEAD"
fi
git worktree add --detach "$RUNNER_TEMP/baseline" HEAD^1
`;
const PR_STEP = {
  if: "github.event_name == 'merge_group'",
  env: {
    GH_TOKEN: gh("github.token"),
    GROUP_REF: gh("github.event.merge_group.head_ref"),
  },
  run: `number=$(printf '%s\\n' "$GROUP_REF" | sed -nE 's#^refs/heads/gh-readonly-queue/.+/pr-([0-9]+)-[0-9a-f]+$#\\1#p')
test -n "$number"
git fetch --no-tags --quiet origin "refs/pull/$number/head"
echo "head=$(git rev-parse FETCH_HEAD)" >> "$GITHUB_OUTPUT"
skip=$(gh api "repos/$GITHUB_REPOSITORY/pulls/$number" --jq 'any(.labels[]; .name == "skip-changes")')
echo "skip=$skip" >> "$GITHUB_OUTPUT"
`,
};
const TEST_CHANGES_ENV = {
  BASE: gh("github.event.pull_request.base.sha || 'HEAD^1'"),
  HEAD: gh("github.event.pull_request.head.sha || 'HEAD'"),
  PR_HEAD: gh("github.event.pull_request.head.sha || steps.pr.outputs.head"),
};
const TEST_CHANGES_RUN =
  'node scripts/check-test-changes.mjs --base "$BASE" --head "$HEAD" --pr-head "$PR_HEAD"';
const PACKAGES_RUN =
  'node scripts/release/semver-packages.mjs --baseline "$RUNNER_TEMP/baseline" >> "$GITHUB_OUTPUT"';
const SEMVER_IF = "steps.packages.outputs.any == 'true'";
const SEMVER_NOTICE = {
  if: "steps.semver.outcome == 'failure'",
  run: 'echo "::warning title=cargo-semver-checks failed::Its log above shows which item broke. If it shows no failed lint, the tool itself failed (a download or build error): re-run the job instead of adding a breaking fragment."',
};
const API_RUN = `set +e
node scripts/release/api-report.mjs --base HEAD^1
code=$?
set -e
echo "breaking=$([ "$code" = 3 ] && echo true || echo false)" >> "$GITHUB_OUTPUT"
[ "$code" = 0 ] || [ "$code" = 3 ]
`;
const CONCURRENCY = {
  group: `changes-${gh("github.ref")}`,
  "cancel-in-progress": true,
};
const TIMEOUT_MINUTES = 15;

const checks = [
  {
    name: "runs on pull_request with the five types incl. labeled and unlabeled, and in the merge queue",
    check: (ctx) => assert.deepEqual(workflow(ctx).on, ON),
    mutants: {
      "drops merge_group": (ctx) => ctx.doc.deleteIn(["on", "merge_group"]),
      "drops the merge_group types": (ctx) =>
        ctx.doc.setIn(["on", "merge_group"], null),
      "drops labeled": (ctx) =>
        ctx.doc.setIn(
          ["on", "pull_request", "types"],
          TYPES.filter((type) => type !== "labeled"),
        ),
      "drops unlabeled": (ctx) =>
        ctx.doc.setIn(
          ["on", "pull_request", "types"],
          TYPES.filter((type) => type !== "unlabeled"),
        ),
      "adds pull_request_target": (ctx) =>
        ctx.doc.setIn(["on", "pull_request_target"], null),
      "replaces the types by null": (ctx) =>
        ctx.doc.setIn(["on", "pull_request"], null),
    },
  },
  {
    name: "grants contents: read and pull-requests: read and nothing else, on the workflow, none on the job",
    check: (ctx) => {
      const wf = workflow(ctx);
      assert.deepEqual(wf.permissions, {
        contents: "read",
        "pull-requests": "read",
      });
      for (const [id, job] of Object.entries(wf.jobs)) {
        assert.equal(job.permissions, undefined, `job ${id} sets permissions`);
      }
    },
    mutants: {
      "drops the permissions": (ctx) => ctx.doc.deleteIn(["permissions"]),
      "drops pull-requests: read": (ctx) =>
        ctx.doc.deleteIn(["permissions", "pull-requests"]),
      "adds pull-requests: write": (ctx) =>
        ctx.doc.setIn(["permissions", "pull-requests"], "write"),
      "sets job permissions": (ctx) =>
        ctx.doc.setIn(["jobs", "changes", "permissions"], {
          contents: "read",
        }),
    },
  },
  {
    name: "reports one job with the id changes and no display name",
    check: (ctx) => {
      const { jobs } = workflow(ctx);
      assert.deepEqual(Object.keys(jobs), ["changes"]);
      assert.equal(jobs.changes.name, undefined, "the job has a display name");
    },
    mutants: {
      "renames the job": (ctx) => {
        const job = ctx.doc.getIn(["jobs", "changes"]);
        ctx.doc.deleteIn(["jobs", "changes"]);
        ctx.doc.setIn(["jobs", "fragments"], job);
      },
      "adds a display name": (ctx) =>
        ctx.doc.setIn(["jobs", "changes", "name"], "Required fragment"),
      "adds a second job": (ctx) =>
        ctx.doc.setIn(["jobs", "lint"], { "runs-on": "ubuntu-latest" }),
    },
  },
  {
    name: "pins every action by full commit SHA with its version as a comment",
    check: (ctx) => {
      const uses = ctx.doc
        .getIn(STEPS, true)
        .items.map((step) => step.get("uses", true))
        .filter(Boolean);
      assert.ok(uses.length > 0, "the workflow uses no action");
      for (const node of uses) {
        assert.match(node.value, /^[\w.-]+\/[\w.-]+@[0-9a-f]{40}$/);
        // cargo-semver-checks-action tags its releases vX.Y only.
        assert.match(node.comment ?? "", /^ v\d+\.\d+(?:\.\d+)?$/, node.value);
      }
    },
    mutants: {
      "pins checkout by tag": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, checkoutStep(ctx), "uses"],
          "actions/checkout@v7",
        ),
      "drops a version comment": (ctx) => {
        ctx.doc.getIn([...STEPS, checkoutStep(ctx), "uses"], true).comment =
          undefined;
      },
    },
  },
  {
    name: "checks out the full history",
    check: (ctx) =>
      assert.equal(steps(ctx)[checkoutStep(ctx)].with?.["fetch-depth"], 0),
    mutants: {
      "deletes fetch-depth": (ctx) =>
        ctx.doc.deleteIn([...STEPS, checkoutStep(ctx), "with", "fetch-depth"]),
      "fetches depth 1": (ctx) =>
        ctx.doc.setIn([...STEPS, checkoutStep(ctx), "with", "fetch-depth"], 1),
    },
  },
  {
    name: "runs check-fragments with the PR base and head and the label-derived --skip",
    check: (ctx) => {
      const step = steps(ctx)[fragmentsStep(ctx)];
      assert.deepEqual(step.env, FRAGMENTS_ENV);
      assert.equal(step.run, FRAGMENTS_RUN);
    },
    mutants: {
      "reads the label only from the pull request event": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, fragmentsStep(ctx), "env", "SKIP"],
          gh(
            "contains(github.event.pull_request.labels.*.name, 'skip-changes') && '--skip' || ''",
          ),
        ),
      "hard-codes SKIP to --skip": (ctx) =>
        ctx.doc.setIn([...STEPS, fragmentsStep(ctx), "env", "SKIP"], "--skip"),
      "diffs from the event's base tip": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, fragmentsStep(ctx), "run"],
          FRAGMENTS_RUN.replace("--base HEAD^1", '--base "$BASE"'),
        ),
      "sets BASE in the env": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, fragmentsStep(ctx), "env", "BASE"],
          gh("github.event.pull_request.base.sha"),
        ),
      "drops $SKIP from the command": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, fragmentsStep(ctx), "run"],
          FRAGMENTS_RUN.replace(" $SKIP", ""),
        ),
    },
  },
  {
    name: "the semver and API steps run before the required-fragment step and feed --require-breaking",
    check: (ctx) => {
      const all = steps(ctx);
      const [baseline, toolchain, semver, api, fragments] = [
        baselineStep(ctx),
        toolchainStep(ctx),
        semverStep(ctx),
        apiStep(ctx),
        fragmentsStep(ctx),
      ];
      assert.ok(baseline < semver, "the baseline is checked out after semver");
      const packages = packagesStep(ctx);
      assert.ok(
        baseline < packages && packages < semver,
        "the packages are listed outside baseline..semver",
      );
      assert.equal(all[packages].run, PACKAGES_RUN);
      assert.ok(toolchain < semver, "the toolchain is installed after semver");
      assert.ok(semver < api, "semver runs after the API report");
      assert.ok(api < fragments, "the API report runs after check-fragments");
      const install = all.findIndex((step) =>
        String(step.run).startsWith("pnpm install"),
      );
      assert.ok(
        install !== -1 && install < api,
        "api runs before pnpm install",
      );

      assert.deepEqual(all[baseline].env, BASELINE_ENV);
      assert.equal(all[baseline].run, BASELINE_RUN);

      const sv = all[semver];
      assert.ok(sv.uses?.startsWith(SEMVER_ACTION), "semver uses no action");
      assert.deepEqual(sv.with, {
        "baseline-root": `${gh("runner.temp")}/baseline`,
        "rust-toolchain": "manual",
        package: gh("steps.packages.outputs.packages"),
      });
      assert.equal(sv.if, SEMVER_IF);
      assert.equal(sv["continue-on-error"], true);

      const report = all[api];
      assert.equal(report.env, undefined);
      assert.equal(report.run, API_RUN);
      assert.equal(report["continue-on-error"], undefined);

      const notice = semverNoticeStep(ctx);
      assert.ok(semver < notice && notice < fragments, "notice out of order");
      assert.equal(all[notice].if, SEMVER_NOTICE.if);
      assert.equal(all[notice].run, SEMVER_NOTICE.run);

      const step = all[fragments];
      assert.equal(step.env.BREAKING, FRAGMENTS_ENV.BREAKING);
      assert.ok(step.run.endsWith(" $SKIP $BREAKING"), step.run);
    },
    mutants: {
      "drops BREAKING from the env": (ctx) =>
        ctx.doc.deleteIn([...STEPS, fragmentsStep(ctx), "env", "BREAKING"]),
      "hard-codes BREAKING to ''": (ctx) =>
        ctx.doc.setIn([...STEPS, fragmentsStep(ctx), "env", "BREAKING"], ""),
      "reads only the semver outcome": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, fragmentsStep(ctx), "env", "BREAKING"],
          gh("steps.semver.outcome == 'failure' && '--require-breaking' || ''"),
        ),
      "drops $BREAKING from the command": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, fragmentsStep(ctx), "run"],
          FRAGMENTS_RUN.replace(" $BREAKING", ""),
        ),
      "moves the required-fragment step before api": (ctx) => {
        const list = ctx.doc.getIn(STEPS);
        const step = list.items.splice(fragmentsStep(ctx), 1)[0];
        list.items.splice(apiStep(ctx), 0, step);
      },
      "semver checks every publishable crate, new ones included": (ctx) =>
        ctx.doc.deleteIn([...STEPS, semverStep(ctx), "with", "package"]),
      "semver runs with no crate to check": (ctx) =>
        ctx.doc.deleteIn([...STEPS, semverStep(ctx), "if"]),
      "the packages are listed against the head, not the base": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, packagesStep(ctx), "run"],
          PACKAGES_RUN.replace('"$RUNNER_TEMP/baseline"', "."),
        ),
      "the package list goes to the log, not the step outputs": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, packagesStep(ctx), "run"],
          PACKAGES_RUN.replace(' >> "$GITHUB_OUTPUT"', ""),
        ),
      "semver without continue-on-error": (ctx) =>
        ctx.doc.deleteIn([...STEPS, semverStep(ctx), "continue-on-error"]),
      "semver against baseline-rev (an archive the repository's symlink breaks)":
        (ctx) => {
          ctx.doc.deleteIn([
            ...STEPS,
            semverStep(ctx),
            "with",
            "baseline-root",
          ]);
          ctx.doc.setIn(
            [...STEPS, semverStep(ctx), "with", "baseline-rev"],
            gh("github.event.pull_request.base.sha"),
          );
        },
      "the baseline is the head": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, baselineStep(ctx), "run"],
          BASELINE_RUN.replace("HEAD^1", "HEAD"),
        ),
      "the baseline step does not check the merge ref": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, baselineStep(ctx), "run"],
          BASELINE_RUN.replace(
            '  test "$(git rev-parse HEAD^2)" = "$HEAD"\n',
            "",
          ),
        ),
      "the baseline step does not check the queue's commit": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, baselineStep(ctx), "run"],
          BASELINE_RUN.replace(
            '  test "$(git rev-parse HEAD)" = "$GROUP_HEAD"\n',
            "  :\n",
          ),
        ),
      "the baseline step checks the merge ref in the queue too": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, baselineStep(ctx), "env", "EVENT"],
          "pull_request",
        ),
      "the baseline step has no HEAD": (ctx) =>
        ctx.doc.deleteIn([...STEPS, baselineStep(ctx), "env", "HEAD"]),
      "the baseline is the merge base of the event's base and head": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, baselineStep(ctx), "run"],
          BASELINE_RUN.replace("HEAD^1", '"$(git merge-base "$BASE" "$HEAD")"'),
        ),
      "the API report diffs against the event's base tip": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, apiStep(ctx), "run"],
          API_RUN.replace("--base HEAD^1", '--base "$BASE"'),
        ),
      "the API report diffs against the merge base": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, apiStep(ctx), "run"],
          API_RUN.replace(
            "--base HEAD^1",
            '--base "$(git merge-base "$BASE" "$HEAD")"',
          ),
        ),
      "the baseline step comes after semver": (ctx) => {
        const list = ctx.doc.getIn(STEPS);
        const step = list.items.splice(baselineStep(ctx), 1)[0];
        list.items.splice(semverStep(ctx) + 1, 0, step);
      },
      "semver with the action's own toolchain": (ctx) =>
        ctx.doc.deleteIn([...STEPS, semverStep(ctx), "with", "rust-toolchain"]),
      "semver before the toolchain install": (ctx) => {
        const list = ctx.doc.getIn(STEPS);
        const step = list.items.splice(toolchainStep(ctx), 1)[0];
        list.items.splice(semverStep(ctx) + 1, 0, step);
      },
      "api lets every exit code pass": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, apiStep(ctx), "run"],
          API_RUN.replace('[ "$code" = 0 ] || [ "$code" = 3 ]\n', ""),
        ),
      "api never reports breaking": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, apiStep(ctx), "run"],
          API_RUN.replace(
            '"$code" = 3 ] && echo true',
            '"$code" = 9 ] && echo true',
          ),
        ),
      "the semver notice runs always": (ctx) =>
        ctx.doc.deleteIn([...STEPS, semverNoticeStep(ctx), "if"]),
      "the semver notice after the fragment step": (ctx) => {
        const list = ctx.doc.getIn(STEPS);
        const step = list.items.splice(semverNoticeStep(ctx), 1)[0];
        list.items.push(step);
      },
      "api with continue-on-error": (ctx) =>
        ctx.doc.setIn([...STEPS, apiStep(ctx), "continue-on-error"], true),
    },
  },
  {
    name: "runs the test-change check last, with the PR base and head, after a failed step too",
    check: (ctx) => {
      const index = testChangesStep(ctx);
      const step = steps(ctx)[index];
      assert.equal(index, steps(ctx).length - 1, "it is not the last step");
      assert.deepEqual(step.env, TEST_CHANGES_ENV);
      assert.equal(step.run, TEST_CHANGES_RUN);
      assert.equal(step.if, gh("!cancelled()"));
      assert.equal(step["continue-on-error"], undefined);
    },
    mutants: {
      "drops --base": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, testChangesStep(ctx), "run"],
          TEST_CHANGES_RUN.replace(' --base "$BASE"', ""),
        ),
      "drops --pr-head": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, testChangesStep(ctx), "run"],
          TEST_CHANGES_RUN.replace(' --pr-head "$PR_HEAD"', ""),
        ),
      "uses the merge group's base_sha in the queue": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, testChangesStep(ctx), "env", "BASE"],
          gh(
            "github.event.pull_request.base.sha || github.event.merge_group.base_sha",
          ),
        ),
      "reads the trailers from the queue's commit": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, testChangesStep(ctx), "env", "PR_HEAD"],
          gh("github.event.pull_request.head.sha || 'HEAD'"),
        ),
      "sets BASE to github.sha": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, testChangesStep(ctx), "env", "BASE"],
          gh("github.sha"),
        ),
      "may fail without failing the job": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, testChangesStep(ctx), "continue-on-error"],
          true,
        ),
      "runs only when every step before it passed": (ctx) =>
        ctx.doc.deleteIn([...STEPS, testChangesStep(ctx), "if"]),
      "runs before the fragment step": (ctx) => {
        const list = ctx.doc.getIn(STEPS);
        const step = list.items.splice(testChangesStep(ctx), 1)[0];
        list.items.splice(fragmentsStep(ctx), 0, step);
      },
    },
  },
  {
    name: "in the merge queue, finds the PR from the queue's branch, fetches its head and reads its label, before the baseline",
    check: (ctx) => {
      const index = prStep(ctx);
      const step = steps(ctx)[index];
      assert.ok(checkoutStep(ctx) < index, "the PR is found before checkout");
      assert.ok(
        index < baselineStep(ctx),
        "the PR is found after the baseline",
      );
      assert.equal(step.if, PR_STEP.if);
      assert.deepEqual(step.env, PR_STEP.env);
      assert.equal(step.run, PR_STEP.run);
      assert.equal(step["continue-on-error"], undefined);
    },
    mutants: {
      "runs on every event": (ctx) =>
        ctx.doc.deleteIn([...STEPS, prStep(ctx), "if"]),
      "may fail without failing the job": (ctx) =>
        ctx.doc.setIn([...STEPS, prStep(ctx), "continue-on-error"], true),
      "accepts a branch name without a PR number": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, prStep(ctx), "run"],
          PR_STEP.run.replace('test -n "$number"\n', ""),
        ),
      "never reads the label": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, prStep(ctx), "run"],
          PR_STEP.run.replace(/skip=.*\n.*\n$/, ""),
        ),
      "runs after the baseline": (ctx) => {
        const list = ctx.doc.getIn(STEPS);
        const step = list.items.splice(prStep(ctx), 1)[0];
        list.items.splice(baselineStep(ctx) + 1, 0, step);
      },
    },
  },
  {
    name: "cancels superseded runs per ref",
    check: (ctx) => assert.deepEqual(workflow(ctx).concurrency, CONCURRENCY),
    mutants: {
      "does not cancel": (ctx) =>
        ctx.doc.setIn(["concurrency", "cancel-in-progress"], false),
      "drops the concurrency block": (ctx) => ctx.doc.deleteIn(["concurrency"]),
    },
  },
  {
    name: "bounds the job in time",
    check: (ctx) =>
      assert.equal(
        workflow(ctx).jobs.changes["timeout-minutes"],
        TIMEOUT_MINUTES,
      ),
    mutants: {
      "deletes the timeout": (ctx) =>
        ctx.doc.deleteIn(["jobs", "changes", "timeout-minutes"]),
      "sets the timeout to 0": (ctx) =>
        ctx.doc.setIn(["jobs", "changes", "timeout-minutes"], 0),
    },
  },
];

runWorkflowChecks({ files: load(), clone, checks });

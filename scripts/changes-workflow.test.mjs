// Asserts the invariants of .github/workflows/changes.yml, the `changes`
// check (epic mt08.q8rt §12): it runs on every pull request event that can
// change its answer, labeled and unlabeled included, so adding or removing
// the skip-changes label re-runs it; it reads only; it reports one job named
// `changes`; its actions are pinned by SHA; it fetches the history the merge
// base needs; it passes the PR's base and head and the label-derived --skip
// to check-fragments; superseded runs are cancelled and the job is bounded.
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

/** A GitHub Actions expression, `${{ … }}`, as the workflow spells it. */
const gh = (expression) => `\${{ ${expression} }}`;

const TYPES = ["opened", "synchronize", "reopened", "labeled", "unlabeled"];
const FRAGMENTS_ENV = {
  BASE: gh("github.event.pull_request.base.sha"),
  HEAD: gh("github.event.pull_request.head.sha"),
  SKIP: gh(
    "contains(github.event.pull_request.labels.*.name, 'skip-changes') && '--skip' || ''",
  ),
};
const FRAGMENTS_RUN =
  'node scripts/release/check-fragments.mjs --base "$BASE" --head "$HEAD" $SKIP';
const CONCURRENCY = {
  group: `changes-${gh("github.ref")}`,
  "cancel-in-progress": true,
};
const TIMEOUT_MINUTES = 15;

const checks = [
  {
    name: "runs on pull_request with the five types incl. labeled and unlabeled",
    check: (ctx) =>
      assert.deepEqual(workflow(ctx).on, { pull_request: { types: TYPES } }),
    mutants: {
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
    name: "grants contents: read and nothing else, on the workflow, none on the job",
    check: (ctx) => {
      const wf = workflow(ctx);
      assert.deepEqual(wf.permissions, { contents: "read" });
      for (const [id, job] of Object.entries(wf.jobs)) {
        assert.equal(job.permissions, undefined, `job ${id} sets permissions`);
      }
    },
    mutants: {
      "drops the permissions": (ctx) => ctx.doc.deleteIn(["permissions"]),
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
        assert.match(node.comment ?? "", /^ v\d+\.\d+\.\d+$/, node.value);
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
      "hard-codes SKIP to --skip": (ctx) =>
        ctx.doc.setIn([...STEPS, fragmentsStep(ctx), "env", "SKIP"], "--skip"),
      "sets BASE to github.sha": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, fragmentsStep(ctx), "env", "BASE"],
          gh("github.sha"),
        ),
      "drops $SKIP from the command": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, fragmentsStep(ctx), "run"],
          FRAGMENTS_RUN.replace(" $SKIP", ""),
        ),
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

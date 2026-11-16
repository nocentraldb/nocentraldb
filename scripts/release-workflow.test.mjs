// Asserts the rules of .github/workflows/release.yml (epic mt08.q8rt §11,
// docs/release-management.md "Alpha", "Beta" and "Release") that hold for the
// workflow as a whole and for every job: the triggers; read-only by default,
// contents: write on the jobs alpha, publish and commit-back only, verify and
// build inheriting read; every step of every job runs in bash with pipefail;
// every action is SHA-pinned; every job is bounded in time; a step that
// derives the basic-auth header from the token masks it before git sees it.
// The alpha job's own rules are in scripts/release-workflow-alpha.test.mjs,
// those of alpha-npm in scripts/release-workflow-npm.test.mjs,
// those of the publish side in scripts/release-workflow-publish.test.mjs.
//
// The workflow is parsed with the `yaml` package and checked by structure.
// Every check is also run against mutants of the file and must reject each
// one (scripts/lib/workflow-checks.mjs). The helpers these files share are in
// scripts/lib/release-workflow.mjs.
import assert from "node:assert/strict";
import {
  ALPHA,
  checkoutStep,
  clone,
  editRun,
  JOBS,
  job,
  load,
  STEPS,
  stepAt,
  stepsOf,
  usesAction,
  WRITE,
  workflow,
} from "./lib/release-workflow.mjs";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const TIMEOUTS = {
  alpha: 20,
  "alpha-npm": 45,
  verify: 45,
  build: 45,
  publish: 20,
  "commit-back": 15,
  "app-token-check": 5,
};
/** The one way a step derives the basic-auth header from GH_TOKEN. */
const AUTH = `auth="$(printf 'x-access-token:%s' "$GH_TOKEN" | base64 -w0)"`;

const checks = [
  {
    name: "triggers: workflow_run of ci on main (completed), push of v* tags, and a dispatch with the single input tag",
    check: (ctx) => {
      const on = workflow(ctx).on;
      assert.deepEqual(Object.keys(on).sort(), [
        "push",
        "workflow_dispatch",
        "workflow_run",
      ]);
      assert.deepEqual(on.workflow_run, {
        workflows: ["ci"],
        types: ["completed"],
        branches: ["main"],
      });
      assert.deepEqual(on.push, { tags: ["v*"] });
      const inputs = on.workflow_dispatch?.inputs ?? {};
      assert.deepEqual(Object.keys(inputs), ["tag"]);
      assert.equal(inputs.tag.required, false);
      assert.equal(inputs.tag.default, "");
    },
    mutants: {
      "drops branches": (ctx) =>
        ctx.doc.deleteIn(["on", "workflow_run", "branches"]),
      "adds pull_request": (ctx) => ctx.doc.setIn(["on", "pull_request"], null),
      "follows ci and changes": (ctx) =>
        ctx.doc.setIn(["on", "workflow_run", "workflows"], ["ci", "changes"]),
      "renames the dispatch input": (ctx) => {
        const input = ctx.doc.getIn([
          "on",
          "workflow_dispatch",
          "inputs",
          "tag",
        ]);
        ctx.doc.deleteIn(["on", "workflow_dispatch", "inputs", "tag"]);
        ctx.doc.setIn(["on", "workflow_dispatch", "inputs", "ref"], input);
      },
      "defaults the input to a tag": (ctx) =>
        ctx.doc.setIn(
          ["on", "workflow_dispatch", "inputs", "tag", "default"],
          "v0.1.0",
        ),
    },
  },
  {
    name: "grants contents: read on the workflow and sets permissions per job",
    check: (ctx) => {
      const wf = workflow(ctx);
      assert.deepEqual(wf.permissions, { contents: "read" });
      assert.deepEqual(Object.keys(wf.jobs).sort(), [...JOBS].sort());
      assert.deepEqual(wf.jobs.alpha.permissions, WRITE);
      // The one job that publishes to npm holds the OIDC token npm's
      // Trusted Publishing exchanges, and no contents: write (NOC-281).
      assert.deepEqual(wf.jobs["alpha-npm"].permissions, {
        contents: "read",
        "id-token": "write",
      });
      // verify and build inherit contents: read from the workflow.
      assert.equal(wf.jobs.verify.permissions, undefined);
      assert.equal(wf.jobs.build.permissions, undefined);
      // Exactly contents: write; id-token and attestations come with the
      // registry publishing, which changes this check.
      assert.deepEqual(wf.jobs.publish.permissions, WRITE);
      // commit-back pushes with the release app's token, not github.token,
      // so it inherits contents: read (scripts/release-ruleset.test.mjs).
      assert.equal(wf.jobs["commit-back"].permissions, undefined);
      // The dispatch-only check of the release app needs no github.token
      // permission of its own either.
      assert.equal(wf.jobs["app-token-check"].permissions, undefined);
    },
    mutants: {
      "grants write at the top": (ctx) =>
        ctx.doc.setIn(["permissions", "contents"], "write"),
      "drops the alpha permissions": (ctx) =>
        ctx.doc.deleteIn([...ALPHA, "permissions"]),
      "grants the alpha id-token: write": (ctx) =>
        ctx.doc.setIn([...ALPHA, "permissions", "id-token"], "write"),
      "grants alpha-npm contents: write": (ctx) =>
        ctx.doc.setIn(
          ["jobs", "alpha-npm", "permissions", "contents"],
          "write",
        ),
      "grants verify contents: write": (ctx) =>
        ctx.doc.setIn(["jobs", "verify", "permissions"], WRITE),
      "grants build contents: write": (ctx) =>
        ctx.doc.setIn(["jobs", "build", "permissions"], WRITE),
      "grants publish id-token: write": (ctx) =>
        ctx.doc.setIn(["jobs", "publish", "permissions", "id-token"], "write"),
      "grants commit-back contents: write": (ctx) =>
        ctx.doc.setIn(["jobs", "commit-back", "permissions"], WRITE),
      "drops the publish permissions": (ctx) =>
        ctx.doc.deleteIn(["jobs", "publish", "permissions"]),
      "adds a job": (ctx) =>
        ctx.doc.setIn(["jobs", "extra"], { "runs-on": "ubuntu-latest" }),
    },
  },
  {
    name: "runs every step of every job in bash with pipefail",
    check: (ctx) => {
      for (const id of JOBS)
        assert.equal(job(ctx, id).defaults?.run?.shell, "bash", id);
    },
    mutants: {
      "drops the defaults": (ctx) => ctx.doc.deleteIn([...ALPHA, "defaults"]),
      "runs sh": (ctx) =>
        ctx.doc.setIn([...ALPHA, "defaults", "run", "shell"], "sh"),
      "drops the defaults of verify": (ctx) =>
        ctx.doc.deleteIn(["jobs", "verify", "defaults"]),
      "drops the defaults of build": (ctx) =>
        ctx.doc.deleteIn(["jobs", "build", "defaults"]),
      "drops the defaults of publish": (ctx) =>
        ctx.doc.deleteIn(["jobs", "publish", "defaults"]),
      "drops the defaults of commit-back": (ctx) =>
        ctx.doc.deleteIn(["jobs", "commit-back", "defaults"]),
    },
  },
  {
    name: "pins every action by full commit SHA with its version as a comment",
    check: (ctx) => {
      const uses = JOBS.flatMap((id) =>
        ctx.doc
          .getIn(["jobs", id, "steps"], true)
          .items.map((step) => step.get("uses", true))
          .filter(Boolean),
      );
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
      "pins upload-artifact by tag": (ctx) =>
        ctx.doc.setIn(
          [
            "jobs",
            "build",
            "steps",
            stepAt(ctx, "build", usesAction("actions/upload-artifact")),
            "uses",
          ],
          "actions/upload-artifact@v7",
        ),
      "pins the commit-back checkout by tag": (ctx) =>
        ctx.doc.setIn(
          [
            "jobs",
            "commit-back",
            "steps",
            stepAt(ctx, "commit-back", usesAction("actions/checkout")),
            "uses",
          ],
          "actions/checkout@v7",
        ),
    },
  },
  {
    name: "bounds every job in time",
    check: (ctx) => {
      for (const id of JOBS)
        assert.equal(job(ctx, id)["timeout-minutes"], TIMEOUTS[id], id);
    },
    mutants: {
      "deletes the timeout": (ctx) =>
        ctx.doc.deleteIn([...ALPHA, "timeout-minutes"]),
      "sets the timeout to 0": (ctx) =>
        ctx.doc.setIn([...ALPHA, "timeout-minutes"], 0),
      "deletes the timeout of verify": (ctx) =>
        ctx.doc.deleteIn(["jobs", "verify", "timeout-minutes"]),
      "deletes the timeout of build": (ctx) =>
        ctx.doc.deleteIn(["jobs", "build", "timeout-minutes"]),
      "deletes the timeout of publish": (ctx) =>
        ctx.doc.deleteIn(["jobs", "publish", "timeout-minutes"]),
      "deletes the timeout of commit-back": (ctx) =>
        ctx.doc.deleteIn(["jobs", "commit-back", "timeout-minutes"]),
    },
  },
  {
    name: "every step that derives the basic-auth header from the token masks it before git can see it",
    check: (ctx) => {
      const derives = JOBS.flatMap((id) =>
        stepsOf(ctx, id)
          .filter((step) => /GIT_CONFIG_VALUE_0|base64/.test(step.run ?? ""))
          .map((step) => [id, String(step.run)]),
      );
      assert.deepEqual(
        derives.map(([id]) => id),
        ["alpha", "commit-back"],
        "the steps that derive the header",
      );
      for (const [id, run] of derives) {
        const derive = run.indexOf(AUTH);
        const mask = run.indexOf('echo "::add-mask::$auth"');
        const use = run.indexOf(
          'GIT_CONFIG_VALUE_0="Authorization: basic $auth"',
        );
        assert.notEqual(
          derive,
          -1,
          `${id}: the header is not derived into auth`,
        );
        assert.ok(
          mask > derive,
          `${id}: auth is not masked after it is derived`,
        );
        assert.ok(
          use > mask,
          `${id}: the header is exported before it is masked`,
        );
        assert.equal(
          run.match(/base64/g).length,
          1,
          `${id}: a second base64 value is derived`,
        );
      }
    },
    mutants: {
      "drops the guard's mask": (ctx) =>
        editRun(ctx, "alpha", "ls-remote", 'echo "::add-mask::$auth"\n', ""),
      "drops commit-back's mask": (ctx) =>
        editRun(
          ctx,
          "commit-back",
          "commit-back.mjs",
          'echo "::add-mask::$auth"\n',
          "",
        ),
      "masks the token instead": (ctx) =>
        editRun(
          ctx,
          "commit-back",
          "commit-back.mjs",
          'echo "::add-mask::$auth"',
          'echo "::add-mask::$GH_TOKEN"',
        ),
      "derives the header inline, unmasked": (ctx) =>
        editRun(
          ctx,
          "commit-back",
          "commit-back.mjs",
          'GIT_CONFIG_VALUE_0="Authorization: basic $auth"',
          `GIT_CONFIG_VALUE_0="Authorization: basic $(printf 'x-access-token:%s' "$GH_TOKEN" | base64 -w0)"`,
        ),
      "exports the header before the mask": (ctx) =>
        editRun(
          ctx,
          "alpha",
          "ls-remote",
          'echo "::add-mask::$auth"\n',
          'export GIT_CONFIG_VALUE_0="Authorization: basic $auth"\necho "::add-mask::$auth"\n',
        ),
    },
  },
];

runWorkflowChecks({ files: load(), clone, checks });

// Asserts .github/workflows/site-pages.yml (NOC-213): the deploy of the site
// (site/, Astro and Starlight) to GitHub Pages under nocentraldb.io. It runs
// on a push to main that changes the site or the workflow, and when a person
// starts it; it has only the permissions Pages needs; one deploy runs at a
// time and none is cancelled; it builds the site, checks the built output
// with site/scripts/output.test.mjs before it uploads it, uploads exactly
// site/dist, and deploys in the github-pages environment; and every action is
// pinned by SHA. Each check also rejects its mutants
// (scripts/lib/workflow-checks.mjs).
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const WORKFLOW = ".github/workflows/site-pages.yml";
const text = existsSync(join(root, WORKFLOW))
  ? readFileSync(join(root, WORKFLOW), "utf8")
  : null;

const workflow = (ctx) => {
  assert.ok(ctx.workflow, `${WORKFLOW} is missing`);
  return ctx.workflow;
};

const steps = (ctx) =>
  Object.values(workflow(ctx).jobs ?? {}).flatMap((job) => job.steps ?? []);

const step = (ctx, action) => {
  const found = steps(ctx).find((s) => s.uses?.startsWith(`${action}@`));
  assert.ok(found, `${WORKFLOW} has no step using ${action}`);
  return found;
};

/** The index of the first step of `job` whose `run` is `command`. */
const runIndex = (job, command) =>
  (job.steps ?? []).findIndex((s) => s.run?.trim() === command);

const INSTALL = "pnpm --dir site install --frozen-lockfile";
const BUILD = "pnpm --dir site build";
const CHECK = "node --test 'site/scripts/*.test.mjs'";

runWorkflowChecks({
  files: { workflow: text === null ? null : parse(text), text },
  clone: (files) => structuredClone(files),
  checks: [
    {
      name: "the site deploys on a push to main that changes it, and by hand",
      check: (ctx) => {
        const on = workflow(ctx).on ?? {};
        assert.deepEqual(Object.keys(on).sort(), ["push", "workflow_dispatch"]);
        assert.deepEqual(on.push.branches, ["main"]);
        assert.deepEqual(on.push.paths, ["site/**", WORKFLOW]);
      },
      mutants: {
        "on every branch": (ctx) => {
          delete ctx.workflow.on.push.branches;
        },
        "on pull requests too": (ctx) => {
          ctx.workflow.on.pull_request = null;
        },
        "on every path": (ctx) => {
          delete ctx.workflow.on.push.paths;
        },
      },
    },
    {
      name: "the site deploy has only the permissions Pages needs",
      check: (ctx) => {
        assert.deepEqual(workflow(ctx).permissions, {
          contents: "read",
          pages: "write",
          "id-token": "write",
        });
        for (const [id, job] of Object.entries(workflow(ctx).jobs)) {
          assert.equal(job.permissions, undefined, `job ${id} widens them`);
        }
      },
      mutants: {
        "contents write": (ctx) => {
          ctx.workflow.permissions.contents = "write";
        },
        "a job widens them": (ctx) => {
          ctx.workflow.jobs.deploy.permissions = { contents: "write" };
        },
      },
    },
    {
      name: "one site deploy runs at a time, and none is cancelled",
      check: (ctx) => {
        assert.deepEqual(workflow(ctx).concurrency, {
          group: "pages",
          "cancel-in-progress": false,
        });
      },
      mutants: {
        "cancels a running deploy": (ctx) => {
          ctx.workflow.concurrency["cancel-in-progress"] = true;
        },
        "no concurrency": (ctx) => {
          delete ctx.workflow.concurrency;
        },
      },
    },
    {
      name: "the site is built and its output checked before it is uploaded",
      check: (ctx) => {
        const build = workflow(ctx).jobs?.build;
        assert.ok(build, "no build job");
        const install = runIndex(build, INSTALL);
        const built = runIndex(build, BUILD);
        const checked = runIndex(build, CHECK);
        const upload = (build.steps ?? []).findIndex((s) =>
          s.uses?.startsWith("actions/upload-pages-artifact@"),
        );
        assert.ok(install >= 0, `no step runs ${INSTALL}`);
        assert.ok(built > install, `${BUILD} runs after the install`);
        assert.ok(checked > built, `${CHECK} runs after the build`);
        assert.ok(upload > checked, "the upload follows the check");
        assert.equal(
          step(ctx, "actions/upload-pages-artifact").with?.path,
          "site/dist",
        );
      },
      mutants: {
        "no output check": (ctx) => {
          ctx.workflow.jobs.build.steps = ctx.workflow.jobs.build.steps.filter(
            (s) => s.run?.trim() !== CHECK,
          );
        },
        "an install that may change the lockfile": (ctx) => {
          for (const s of ctx.workflow.jobs.build.steps) {
            if (s.run?.trim() === INSTALL) s.run = "pnpm --dir site install";
          }
        },
        "uploads another folder": (ctx) => {
          step(ctx, "actions/upload-pages-artifact").with.path = "site";
        },
      },
    },
    {
      name: "the site deploys in the github-pages environment after the build",
      check: (ctx) => {
        const deploy = workflow(ctx).jobs?.deploy;
        assert.ok(deploy, "no deploy job");
        assert.equal(deploy.needs, "build");
        assert.equal(deploy.environment?.name, "github-pages");
        assert.ok(
          deploy.steps?.some((s) =>
            s.uses?.startsWith("actions/deploy-pages@"),
          ),
          "the deploy job does not deploy",
        );
      },
      mutants: {
        "deploys without the build": (ctx) => {
          delete ctx.workflow.jobs.deploy.needs;
        },
        "deploys outside the github-pages environment": (ctx) => {
          ctx.workflow.jobs.deploy.environment.name = "production";
        },
      },
    },
    {
      name: "every action in the site deploy is pinned by SHA",
      check: (ctx) => {
        const uses = steps(ctx).flatMap((s) => (s.uses ? [s.uses] : []));
        assert.ok(uses.length > 0, "no action used");
        for (const ref of uses) {
          assert.match(ref, /^[\w.-]+\/[\w.-]+@[0-9a-f]{40}$/, ref);
          const pinned = ctx.text
            .split("\n")
            .find((line) => line.includes(`uses: ${ref}`));
          assert.match(pinned ?? "", / # v\d+\.\d+\.\d+$/, `${ref} comment`);
        }
      },
      mutants: {
        "a tag instead of a SHA": (ctx) => {
          step(ctx, "actions/deploy-pages").uses = "actions/deploy-pages@v5";
        },
        "a SHA without its version comment": (ctx) => {
          ctx.text = ctx.text.replace(/(deploy-pages@[0-9a-f]{40}) #.*/, "$1");
        },
      },
    },
  ],
});

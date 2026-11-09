// Asserts .github/workflows/browser-storage-pages.yml (NOC-33): the deploy
// of the probe page to GitHub Pages that the owner can switch on. It runs
// only when a person starts it, with the least permissions Pages needs,
// deploys exactly the folder build.mjs writes, pins every action by SHA, and
// the device checklist names it. Each check also rejects its mutants
// (scripts/lib/workflow-checks.mjs).
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const WORKFLOW = ".github/workflows/browser-storage-pages.yml";
const CHECKLIST = "docs/browser-storage-checklist.md";
const read = (file) =>
  existsSync(join(root, file)) ? readFileSync(join(root, file), "utf8") : null;

const load = () => {
  const text = read(WORKFLOW);
  return {
    workflow: text === null ? null : parse(text),
    text,
    checklist: read(CHECKLIST) ?? "",
  };
};

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

const PAGES_URL = "https://nocentraldb.github.io/nocentraldb/";

runWorkflowChecks({
  files: load(),
  clone: (files) => structuredClone(files),
  checks: [
    {
      name: "the Pages deploy runs only when a person starts it",
      check: (ctx) => {
        assert.deepEqual(Object.keys(workflow(ctx).on ?? {}), [
          "workflow_dispatch",
        ]);
      },
      mutants: {
        "on push too": (ctx) => {
          ctx.workflow.on.push = { branches: ["main"] };
        },
        "on pull_request instead": (ctx) => {
          ctx.workflow.on = { pull_request: null };
        },
      },
    },
    {
      name: "the Pages deploy has only the permissions Pages needs",
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
      name: "every action in the Pages deploy is pinned by SHA",
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
    {
      name: "the Pages deploy publishes exactly the folder build.mjs writes",
      check: (ctx) => {
        const builds = steps(ctx).filter((s) =>
          /node scripts\/browser-storage\/build\.mjs (\S+)$/.test(
            s.run?.trim() ?? "",
          ),
        );
        assert.equal(builds.length, 1, "one build.mjs step");
        const out = builds[0].run.trim().split(" ").at(-1);
        assert.equal(
          step(ctx, "actions/upload-pages-artifact").with?.path,
          out,
        );
        const deploy = Object.values(workflow(ctx).jobs).find((job) =>
          job.steps?.some((s) => s.uses?.startsWith("actions/deploy-pages@")),
        );
        assert.equal(deploy?.environment?.name, "github-pages");
      },
      mutants: {
        "uploads another folder": (ctx) => {
          step(ctx, "actions/upload-pages-artifact").with.path = "docs";
        },
        "deploys outside the github-pages environment": (ctx) => {
          ctx.workflow.jobs.deploy.environment.name = "production";
        },
      },
    },
    {
      name: "the checklist names the Pages deploy and its address",
      check: (ctx) => {
        assert.ok(ctx.checklist.includes("browser-storage-pages"), "workflow");
        assert.ok(ctx.checklist.includes(PAGES_URL), PAGES_URL);
      },
      mutants: {
        "no address": (ctx) => {
          ctx.checklist = ctx.checklist.replaceAll(PAGES_URL, "");
        },
      },
    },
  ],
});

// The checks every browser workflow shares (NOC-221): it does not depend on
// the runner's apt mirror. Nothing runs `playwright install` or apt; the
// browsers and their system libraries come from Playwright's container
// image, at the version package.json pins and by digest, pulled with a short
// timeout and one retry. Used by scripts/browser-view-workflow.test.mjs and
// scripts/browser-storage-workflow.test.mjs with runWorkflowChecks
// (scripts/lib/workflow-checks.mjs).
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const read = (file) =>
  existsSync(join(root, file)) ? readFileSync(join(root, file), "utf8") : null;

/** The workflow at `file`, parsed, and the playwright version package.json pins. */
export const load = (file) => {
  const text = read(file);
  return {
    file,
    workflow: text === null ? null : parse(text),
    playwright: JSON.parse(read("package.json")).devDependencies?.playwright,
  };
};

export const workflow = (ctx) => {
  assert.ok(ctx.workflow, `${ctx.file} is missing`);
  return ctx.workflow;
};

export const steps = (ctx) =>
  Object.values(workflow(ctx).jobs ?? {}).flatMap((job) => job.steps ?? []);

/** The steps of the workflow's only job, to change in a mutant. */
export const jobSteps = (ctx) => Object.values(workflow(ctx).jobs)[0].steps;

const runs = (ctx) => steps(ctx).flatMap((s) => (s.run ? [s.run] : []));

/** The step whose `run` mentions `text`. */
export const stepWith = (ctx, text) => {
  const found = steps(ctx).find((s) => s.run?.includes(text));
  assert.ok(found, `${ctx.file} has no step that runs ${text}`);
  return found;
};

/** The image the workflow names in `env.PLAYWRIGHT_IMAGE`. */
const image = (ctx) => {
  const value = workflow(ctx).env?.PLAYWRIGHT_IMAGE;
  assert.equal(typeof value, "string", "env.PLAYWRIGHT_IMAGE is not set");
  return value;
};

/** The checks every browser workflow passes. */
export const imageChecks = () => [
  {
    name: "nothing installs browsers or packages from apt",
    check: (ctx) => {
      for (const run of runs(ctx)) {
        assert.doesNotMatch(run, /playwright install/, run);
        assert.doesNotMatch(run, /\bapt(-get)?\b/, run);
      }
    },
    mutants: {
      "playwright install --with-deps again": (ctx) => {
        jobSteps(ctx).push({
          run: "pnpm exec playwright install --with-deps chromium firefox webkit",
        });
      },
      "an apt-get install": (ctx) => {
        jobSteps(ctx).push({ run: "sudo apt-get install -y libnss3" });
      },
    },
  },
  {
    name: "the image is Playwright's, at the pinned version, by digest",
    check: (ctx) => {
      assert.match(ctx.playwright ?? "", /^\d+\.\d+\.\d+$/);
      const escaped = ctx.playwright.replaceAll(".", "\\.");
      assert.match(
        image(ctx),
        new RegExp(
          `^mcr\\.microsoft\\.com/playwright:v${escaped}-[a-z]+@sha256:[0-9a-f]{64}$`,
        ),
      );
    },
    mutants: {
      "another version": (ctx) => {
        ctx.playwright = "1.0.0";
      },
      "no digest": (ctx) => {
        ctx.workflow.env.PLAYWRIGHT_IMAGE = image(ctx).replace(/@.*$/, "");
      },
      "another registry": (ctx) => {
        ctx.workflow.env.PLAYWRIGHT_IMAGE = image(ctx).replace(
          "mcr.microsoft.com/playwright",
          "docker.io/someone/playwright",
        );
      },
      "no image": (ctx) => {
        delete ctx.workflow.env.PLAYWRIGHT_IMAGE;
      },
    },
  },
  {
    name: "the image is pulled with a short timeout and one retry",
    check: (ctx) => {
      const pull = stepWith(ctx, "docker pull");
      assert.ok(
        Number.isInteger(pull["timeout-minutes"]) &&
          pull["timeout-minutes"] <= 10,
        "the pull step has no short timeout-minutes",
      );
      assert.match(pull.run, /timeout \d+s? docker pull "\$PLAYWRIGHT_IMAGE"/);
      assert.match(pull.run, /for attempt in 1 2\b/);
    },
    mutants: {
      "no step timeout": (ctx) => {
        delete stepWith(ctx, "docker pull")["timeout-minutes"];
      },
      "a long step timeout": (ctx) => {
        stepWith(ctx, "docker pull")["timeout-minutes"] = 30;
      },
      "no retry": (ctx) => {
        const pull = stepWith(ctx, "docker pull");
        pull.run = 'timeout 240 docker pull "$PLAYWRIGHT_IMAGE"';
      },
      "no timeout on the pull": (ctx) => {
        const pull = stepWith(ctx, "docker pull");
        pull.run = pull.run.replace(/timeout \d+s? docker pull/, "docker pull");
      },
    },
  },
];

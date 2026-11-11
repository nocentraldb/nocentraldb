// Asserts .github/workflows/browser-view.yml (NOC-221): the browser view
// test (NOC-174) does not depend on the runner's apt mirror. The harness is
// built on the runner, which needs no package from apt; the browsers and
// their system libraries come from Playwright's container image, at the
// version package.json pins and by digest, pulled with a short timeout and
// one retry; nothing runs `playwright install`. Each check also rejects its
// mutants (scripts/lib/workflow-checks.mjs).
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const WORKFLOW = ".github/workflows/browser-view.yml";
const read = (file) =>
  existsSync(join(root, file)) ? readFileSync(join(root, file), "utf8") : null;

const load = () => {
  const text = read(WORKFLOW);
  return {
    workflow: text === null ? null : parse(text),
    playwright: JSON.parse(read("package.json")).devDependencies?.playwright,
  };
};

const workflow = (ctx) => {
  assert.ok(ctx.workflow, `${WORKFLOW} is missing`);
  return ctx.workflow;
};

const steps = (ctx) =>
  Object.values(workflow(ctx).jobs ?? {}).flatMap((job) => job.steps ?? []);

/** The steps of the job `view`, to change in a mutant. */
const viewSteps = (ctx) => workflow(ctx).jobs.view.steps;

const runs = (ctx) => steps(ctx).flatMap((s) => (s.run ? [s.run] : []));

/** The step whose `run` mentions `text`. */
const stepWith = (ctx, text) => {
  const found = steps(ctx).find((s) => s.run?.includes(text));
  assert.ok(found, `${WORKFLOW} has no step that runs ${text}`);
  return found;
};

/** The image the workflow names in `env.PLAYWRIGHT_IMAGE`. */
const image = (ctx) => {
  const value = workflow(ctx).env?.PLAYWRIGHT_IMAGE;
  assert.equal(typeof value, "string", "env.PLAYWRIGHT_IMAGE is not set");
  return value;
};

runWorkflowChecks({
  files: load(),
  clone: (files) => structuredClone(files),
  checks: [
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
          viewSteps(ctx).push({
            run: "pnpm exec playwright install --with-deps chromium firefox webkit",
          });
        },
        "an apt-get install": (ctx) => {
          viewSteps(ctx).push({ run: "sudo apt-get install -y libnss3" });
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
        assert.match(
          pull.run,
          /timeout \d+s? docker pull "\$PLAYWRIGHT_IMAGE"/,
        );
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
          pull.run = pull.run.replace(
            /timeout \d+s? docker pull/,
            "docker pull",
          );
        },
      },
    },
    {
      name: "the tests run in the image on the harness built on the runner",
      check: (ctx) => {
        const all = steps(ctx);
        const build = all.findIndex((s) =>
          s.run?.includes("node scripts/browser-view/build.mjs"),
        );
        const test = all.findIndex((s) => s.run?.includes("docker run"));
        assert.ok(build >= 0, "the harness is not built on the runner");
        assert.ok(test > build, "the tests do not run after the build");
        const run = all[test].run;
        assert.match(run, /"\$PLAYWRIGHT_IMAGE"/);
        assert.match(run, /BROWSER_VIEW_PREBUILT=1/);
        assert.match(run, /node --test 'scripts\/browser-view\/\*\.test\.mjs'/);
      },
      mutants: {
        "no build on the runner": (ctx) => {
          const all = viewSteps(ctx);
          const at = all.findIndex((s) =>
            s.run?.includes("node scripts/browser-view/build.mjs"),
          );
          all.splice(at, 1);
        },
        "the tests rebuild in the image": (ctx) => {
          const run = stepWith(ctx, "docker run");
          run.run = run.run.replace("BROWSER_VIEW_PREBUILT=1", "X=1");
        },
        "the tests on the runner": (ctx) => {
          const run = stepWith(ctx, "docker run");
          run.run = "moon exec repo:browser-view --force";
        },
      },
    },
  ],
});

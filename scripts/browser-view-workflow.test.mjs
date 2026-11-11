// Asserts .github/workflows/browser-view.yml (NOC-221): the browser view
// test (NOC-174) does not depend on the runner's apt mirror
// (scripts/lib/playwright-image-checks.mjs). The harness is built on the
// runner, which needs no package from apt, and the tests run in Playwright's
// image on it. Each check also rejects its mutants
// (scripts/lib/workflow-checks.mjs).
import assert from "node:assert/strict";
import {
  imageChecks,
  jobSteps,
  load,
  steps,
  stepWith,
} from "./lib/playwright-image-checks.mjs";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const BUILD = "node scripts/browser-view/build.mjs";

runWorkflowChecks({
  files: load(".github/workflows/browser-view.yml"),
  clone: (files) => structuredClone(files),
  checks: [
    ...imageChecks(),
    {
      name: "the tests run in the image on the harness built on the runner",
      check: (ctx) => {
        const all = steps(ctx);
        const build = all.findIndex((s) => s.run?.includes(BUILD));
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
          const all = jobSteps(ctx);
          all.splice(
            all.findIndex((s) => s.run?.includes(BUILD)),
            1,
          );
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

// Asserts .github/workflows/browser-view.yml (NOC-221): the browser view
// test (NOC-174) does not depend on the runner's apt mirror
// (scripts/lib/playwright-image-checks.mjs). The harness is built on the
// runner, which needs no package from apt, and the tests run in Playwright's
// image on it. Each check also rejects its mutants
// (scripts/lib/workflow-checks.mjs).
//
// NOC-175 slice 2: the same job runs the SDK's key tests in IndexedDB in
// Chromium (scripts/sdk-keys/), built on the runner as the view's harness
// is, whenever a pull request touches the SDK.
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
const KEYS_BUILD = "node scripts/sdk-keys/build.mjs";
const KEYS_TESTS = "scripts/sdk-keys/*.test.mjs";

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
    {
      name: "the SDK's key tests run in the image, built on the runner, on SDK changes",
      check: (ctx) => {
        const all = steps(ctx);
        const build = all.findIndex((s) => s.run?.includes(KEYS_BUILD));
        const test = all.findIndex(
          (s) => s.run?.includes("docker run") && s.run.includes(KEYS_TESTS),
        );
        assert.ok(
          build >= 0,
          "the SDK's key tests are not built on the runner",
        );
        assert.ok(
          test > build,
          "the SDK's key tests do not run after their build",
        );
        const run = all[test].run;
        assert.match(run, /"\$PLAYWRIGHT_IMAGE"/);
        assert.match(run, /SDK_KEYS_PREBUILT=1/);
        const paths = ctx.workflow.on?.pull_request?.paths ?? [];
        assert.ok(
          paths.includes("crates/nocentraldb/**"),
          "SDK changes do not run it",
        );
        assert.ok(
          paths.includes("scripts/sdk-keys/**"),
          "changes of its test do not run it",
        );
      },
      mutants: {
        "no build of the key tests": (ctx) => {
          const all = jobSteps(ctx);
          all.splice(
            all.findIndex((s) => s.run?.includes(KEYS_BUILD)),
            1,
          );
        },
        "the key tests rebuild in the image": (ctx) => {
          const run = jobSteps(ctx).find((s) => s.run?.includes(KEYS_TESTS));
          run.run = run.run.replace("SDK_KEYS_PREBUILT=1", "X=1");
        },
        "SDK changes do not run it": (ctx) => {
          const paths = ctx.workflow.on.pull_request.paths;
          paths.splice(paths.indexOf("crates/nocentraldb/**"), 1);
        },
        "no key tests at all": (ctx) => {
          const all = jobSteps(ctx);
          all.splice(
            all.findIndex((s) => s.run?.includes(KEYS_TESTS)),
            1,
          );
        },
      },
    },
  ],
});

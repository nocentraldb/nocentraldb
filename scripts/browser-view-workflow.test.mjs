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
//
// NOC-176 slice 1: the same job runs the TypeScript SDK in Chromium
// (scripts/sdk-browser/), its package built on the runner, whenever a pull
// request touches the TypeScript SDK or the crate its core is built from.
//
// The example app apps/tasks-ts: the same job runs it in Chromium, on the
// SDK built above, whenever a pull request touches the app or the SDK.
//
// NOC-319: the same job runs the landing page's live demo in Chromium
// (scripts/site-demo/): the site is built on the runner with the demo's
// assets from the SDK built above, whenever a pull request touches the
// site, the demo's tests or the SDK.
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
const SDK_BUILD = "node scripts/sdk-browser/build.mjs";
const SDK_TESTS = "scripts/sdk-browser/*.test.mjs";
const APP_TESTS = "apps/tasks-ts/e2e/*.e2e.mjs";
const DEMO_BUILD = "node scripts/site-demo/build.mjs";
const DEMO_TESTS = "scripts/site-demo/*.test.mjs";

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
    {
      name: "the TypeScript SDK's browser tests run in the image, built on the runner, on SDK changes",
      check: (ctx) => {
        const all = steps(ctx);
        const build = all.findIndex((s) => s.run?.includes(SDK_BUILD));
        const test = all.findIndex(
          (s) => s.run?.includes("docker run") && s.run.includes(SDK_TESTS),
        );
        assert.ok(build >= 0, "the TypeScript SDK is not built on the runner");
        assert.match(
          all[build].if ?? "",
          /^\$\{\{ !cancelled\(\) \}\}$/,
          "a failed build above skips the TypeScript SDK's build",
        );
        assert.ok(test > build, "its tests do not run after its build");
        const run = all[test].run;
        assert.match(run, /"\$PLAYWRIGHT_IMAGE"/);
        assert.match(run, /SDK_BROWSER_PREBUILT=1/);
        const paths = ctx.workflow.on?.pull_request?.paths ?? [];
        for (const path of [
          "packages/sdk/**",
          "crates/nocentraldb-wasm/**",
          "crates/nocentraldb/**",
          "scripts/sdk-browser/**",
        ]) {
          assert.ok(paths.includes(path), `changes of ${path} do not run it`);
        }
      },
      mutants: {
        "the TypeScript SDK's build skipped after a failure above": (ctx) => {
          const step = jobSteps(ctx).find((s) => s.run?.includes(SDK_BUILD));
          delete step.if;
        },
        "no build of the TypeScript SDK": (ctx) => {
          const all = jobSteps(ctx);
          all.splice(
            all.findIndex((s) => s.run?.includes(SDK_BUILD)),
            1,
          );
        },
        "the SDK's tests rebuild in the image": (ctx) => {
          const run = jobSteps(ctx).find((s) => s.run?.includes(SDK_TESTS));
          run.run = run.run.replace("SDK_BROWSER_PREBUILT=1", "X=1");
        },
        "TypeScript SDK changes do not run it": (ctx) => {
          const paths = ctx.workflow.on.pull_request.paths;
          paths.splice(paths.indexOf("packages/sdk/**"), 1);
        },
        "no TypeScript SDK tests at all": (ctx) => {
          const all = jobSteps(ctx);
          all.splice(
            all.findIndex((s) => s.run?.includes(SDK_TESTS)),
            1,
          );
        },
      },
    },
    {
      name: "the example app runs in the image on the SDK built on the runner, on app and SDK changes",
      check: (ctx) => {
        const all = steps(ctx);
        const build = all.findIndex((s) => s.run?.includes(SDK_BUILD));
        const test = all.findIndex(
          (s) => s.run?.includes("docker run") && s.run.includes(APP_TESTS),
        );
        assert.ok(test >= 0, "the example app does not run");
        assert.ok(test > build, "it does not run after the SDK's build");
        assert.match(
          all[test].if ?? "",
          /^\$\{\{ !cancelled\(\) \}\}$/,
          "a failed step above skips the example app",
        );
        const run = all[test].run;
        assert.match(run, /"\$PLAYWRIGHT_IMAGE"/);
        assert.match(run, /SDK_BROWSER_PREBUILT=1/);
        const paths = ctx.workflow.on?.pull_request?.paths ?? [];
        for (const path of ["apps/tasks-ts/**", "packages/sdk/**"]) {
          assert.ok(paths.includes(path), `changes of ${path} do not run it`);
        }
      },
      mutants: {
        "the example app rebuilds the SDK in the image": (ctx) => {
          const run = jobSteps(ctx).find((s) => s.run?.includes(APP_TESTS));
          run.run = run.run.replace("SDK_BROWSER_PREBUILT=1", "X=1");
        },
        "the example app skipped after a failure above": (ctx) => {
          const step = jobSteps(ctx).find((s) => s.run?.includes(APP_TESTS));
          delete step.if;
        },
        "app changes do not run it": (ctx) => {
          const paths = ctx.workflow.on.pull_request.paths;
          paths.splice(paths.indexOf("apps/tasks-ts/**"), 1);
        },
        "no example app at all": (ctx) => {
          const all = jobSteps(ctx);
          all.splice(
            all.findIndex((s) => s.run?.includes(APP_TESTS)),
            1,
          );
        },
      },
    },
    {
      name: "the landing page's demo runs in the image on the site built on the runner, on site and SDK changes",
      check: (ctx) => {
        const all = steps(ctx);
        const sdk = all.findIndex((s) => s.run?.includes(SDK_BUILD));
        const build = all.findIndex((s) => s.run?.includes(DEMO_BUILD));
        const test = all.findIndex(
          (s) => s.run?.includes("docker run") && s.run.includes(DEMO_TESTS),
        );
        assert.ok(build >= 0, "the site with its demo is not built");
        assert.ok(build > sdk, "the site is built before the SDK");
        assert.equal(
          all[build].env?.SDK_BROWSER_PREBUILT,
          "1",
          "the site's build builds the SDK again",
        );
        assert.ok(test > build, "the demo does not run after the site's build");
        for (const at of [build, test]) {
          assert.match(
            all[at].if ?? "",
            /^\$\{\{ !cancelled\(\) \}\}$/,
            "a failed step above skips the demo",
          );
        }
        const run = all[test].run;
        assert.match(run, /"\$PLAYWRIGHT_IMAGE"/);
        assert.match(run, /SITE_DEMO_PREBUILT=1/);
        const paths = ctx.workflow.on?.pull_request?.paths ?? [];
        for (const path of [
          "site/**",
          "scripts/site-demo/**",
          "packages/sdk/**",
        ]) {
          assert.ok(paths.includes(path), `changes of ${path} do not run it`);
        }
      },
      mutants: {
        "the demo builds the site again in the image": (ctx) => {
          const run = jobSteps(ctx).find((s) => s.run?.includes(DEMO_TESTS));
          run.run = run.run.replace("SITE_DEMO_PREBUILT=1", "X=1");
        },
        "the site's build rebuilds the SDK": (ctx) => {
          const build = jobSteps(ctx).find((s) => s.run?.includes(DEMO_BUILD));
          delete build.env;
        },
        "the demo skipped after a failure above": (ctx) => {
          const step = jobSteps(ctx).find((s) => s.run?.includes(DEMO_TESTS));
          delete step.if;
        },
        "site changes do not run it": (ctx) => {
          const paths = ctx.workflow.on.pull_request.paths;
          paths.splice(paths.indexOf("site/**"), 1);
        },
        "no demo at all": (ctx) => {
          const all = jobSteps(ctx);
          all.splice(
            all.findIndex((s) => s.run?.includes(DEMO_TESTS)),
            1,
          );
        },
      },
    },
  ],
});

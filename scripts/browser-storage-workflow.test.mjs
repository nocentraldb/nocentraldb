// Asserts .github/workflows/browser-storage.yml (NOC-221): the browser
// storage probe (NOC-33) does not depend on the runner's apt mirror
// (scripts/lib/playwright-image-checks.mjs). Its tests and the measured rows
// of the run's summary come from Playwright's image. Each check also rejects
// its mutants (scripts/lib/workflow-checks.mjs).
import assert from "node:assert/strict";
import {
  imageChecks,
  load,
  steps,
  stepWith,
} from "./lib/playwright-image-checks.mjs";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

/** The `docker run` step that runs `command` in the image. */
const inImage = (ctx, command) => {
  const found = steps(ctx).find(
    (s) => s.run?.includes("docker run") && s.run.includes(command),
  );
  assert.ok(found, `${ctx.file} does not run ${command} in the image`);
  assert.match(found.run, /"\$PLAYWRIGHT_IMAGE"/);
  return found;
};

runWorkflowChecks({
  files: load(".github/workflows/browser-storage.yml"),
  clone: (files) => structuredClone(files),
  checks: [
    ...imageChecks(),
    {
      name: "the tests and the measured rows run in the image",
      check: (ctx) => {
        inImage(ctx, "node --test 'scripts/browser-storage/*.test.mjs'");
        const rows = inImage(ctx, "node scripts/browser-storage/run.mjs");
        assert.match(rows.run, /GITHUB_STEP_SUMMARY/);
      },
      mutants: {
        "the tests on the runner": (ctx) => {
          const run = stepWith(ctx, "scripts/browser-storage/*.test.mjs");
          run.run = "node --test 'scripts/browser-storage/*.test.mjs'";
        },
        "the rows on the runner": (ctx) => {
          const run = stepWith(ctx, "scripts/browser-storage/run.mjs");
          run.run = run.run.replace(/docker run[^\n]*\n?/, "");
          run.run = run.run.replace(/"\$PLAYWRIGHT_IMAGE"/g, "");
        },
        "the rows not in the summary": (ctx) => {
          const run = stepWith(ctx, "scripts/browser-storage/run.mjs");
          run.run = run.run.replaceAll("GITHUB_STEP_SUMMARY", "OTHER");
        },
      },
    },
  ],
});

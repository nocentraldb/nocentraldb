// Asserts how the commit-back job of .github/workflows/release.yml reaches
// main under the ruleset "Require PR" (NOC-56): the job pushes its commit
// straight to main as the GitHub Actions app, which the ruleset's bypass
// list lets through. So the push uses the job's own token, github.token,
// never a secret of another actor; no step opens a pull request and the
// job asks for no pull-requests permission; and docs/release-management.md
// has the section that gives the owner the exact settings step for the
// bypass, which only the owner can make.
//
// Every check also runs against mutants of the workflow and the runbook and
// must reject each one (scripts/lib/workflow-checks.mjs).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  editRun,
  gh,
  job,
  load,
  runs,
  stepAt,
  stepsOf,
} from "./lib/release-workflow.mjs";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RUNBOOK_PATH = "docs/release-management.md";
const HEADING = "### The bypass for commit-back";

const files = () => ({
  ...load(),
  runbook: readFileSync(join(root, RUNBOOK_PATH), "utf8"),
});
const copy = (ctx) => ({ doc: ctx.doc.clone(), runbook: ctx.runbook });

/** The runbook section under HEADING, up to the next heading of level 2 or 3. */
const section = (runbook) => {
  const start = runbook.indexOf(`\n${HEADING}\n`);
  assert.notEqual(start, -1, `${RUNBOOK_PATH} has no "${HEADING}"`);
  const body = runbook.slice(start + HEADING.length + 2);
  const end = body.search(/\n#{2,3} /);
  return end === -1 ? body : body.slice(0, end);
};

const checks = [
  {
    name: "commit-back pushes to main with the job's own token, which the ruleset's bypass for the GitHub Actions app lets through",
    check: (ctx) => {
      const step = stepsOf(ctx, "commit-back")[
        stepAt(ctx, "commit-back", runs("commit-back.mjs"))
      ];
      assert.equal(
        step.env?.GH_TOKEN,
        gh("github.token"),
        "the push is not the GitHub Actions app's, so its bypass does not apply",
      );
      assert.doesNotMatch(
        step.run,
        /--no-push\b/,
        "commit-back.mjs does not push",
      );
      for (const other of stepsOf(ctx, "commit-back"))
        assert.doesNotMatch(
          String(other.run ?? ""),
          /\bgh pr\b|\bpulls\b/,
          "a step opens a pull request instead of pushing",
        );
      assert.equal(
        job(ctx, "commit-back").permissions?.["pull-requests"],
        undefined,
        "commit-back asks for pull-requests",
      );
    },
    mutants: {
      "pushes with a secret of another actor": (ctx) =>
        ctx.doc.setIn(
          [
            "jobs",
            "commit-back",
            "steps",
            stepAt(ctx, "commit-back", runs("commit-back.mjs")),
            "env",
            "GH_TOKEN",
          ],
          gh("secrets.RELEASE_TOKEN"),
        ),
      "commits without pushing": (ctx) =>
        editRun(
          ctx,
          "commit-back",
          "commit-back.mjs",
          '--date "$PUBLISHED"',
          '--date "$PUBLISHED" --no-push',
        ),
      "opens a pull request": (ctx) =>
        editRun(
          ctx,
          "commit-back",
          "commit-back.mjs",
          '--date "$PUBLISHED"',
          '--date "$PUBLISHED" --no-push\ngh pr create --fill',
        ),
      "asks for pull-requests": (ctx) =>
        ctx.doc.setIn(
          ["jobs", "commit-back", "permissions", "pull-requests"],
          "write",
        ),
    },
  },
  {
    name: "the runbook gives the owner the exact settings step for the bypass",
    check: (ctx) => {
      const text = section(ctx.runbook);
      for (const needle of [
        "owner",
        "Settings → Rules → Rulesets",
        "Require PR",
        "Add bypass",
        "GitHub Actions",
        "Always allow",
      ])
        assert.ok(text.includes(needle), `the section does not name ${needle}`);
      assert.ok(
        ctx.runbook.includes("(#the-bypass-for-commit-back)"),
        "nothing in the runbook links to the section",
      );
    },
    mutants: {
      "drops the section": (ctx) => {
        ctx.runbook = ctx.runbook.replace(HEADING, "### Something else");
      },
      "names no actor": (ctx) => {
        ctx.runbook = ctx.runbook.replace(
          section(ctx.runbook),
          section(ctx.runbook).replaceAll("GitHub Actions", "the app"),
        );
      },
      "does not say it is the owner's": (ctx) => {
        ctx.runbook = ctx.runbook.replace(
          section(ctx.runbook),
          section(ctx.runbook).replaceAll("owner", "maintainer"),
        );
      },
      "no link to it": (ctx) => {
        ctx.runbook = ctx.runbook.replaceAll(
          "(#the-bypass-for-commit-back)",
          "",
        );
      },
    },
  },
];

runWorkflowChecks({ files: files(), clone: copy, checks });

// Asserts how the commit-back job of .github/workflows/release.yml reaches
// main under the ruleset "Require PR" (NOC-56): the job pushes its commit
// straight to main as the organization's release GitHub App, which is the
// only actor on the ruleset's bypass list. So commit-back mints a
// short-lived token for that app with actions/create-github-app-token,
// pinned by commit SHA, from the repository variable RELEASE_APP_CLIENT_ID
// (the app's Client ID; the action's deprecated app-id is not used) and the
// repository secret RELEASE_APP_PRIVATE_KEY, narrowed to contents: write;
// it checks out and pushes with that token and never with github.token,
// whose job permission stays read-only; no other job mints or reads the
// app's token; no step opens a pull request. docs/release-management.md has
// the section that gives the owner the exact settings steps, which only the
// owner can make.
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
  JOBS,
  job,
  load,
  runs,
  stepAt,
  stepsOf,
  usesAction,
} from "./lib/release-workflow.mjs";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RUNBOOK_PATH = "docs/release-management.md";
const HEADING = "### The bypass for commit-back";
const APP_STEP = "app-token";
const APP_TOKEN = gh(`steps.${APP_STEP}.outputs.token`);
const APP_PATH = (ctx) => [
  "jobs",
  "commit-back",
  "steps",
  stepAt(ctx, "commit-back", (step) => step.id === APP_STEP),
];

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
    name: "commit-back pushes to main with the release app's token, which the ruleset's bypass lets through, and no other job holds that token",
    check: (ctx) => {
      const steps = stepsOf(ctx, "commit-back");
      const mint = stepAt(ctx, "commit-back", (step) => step.id === APP_STEP);
      const token = steps[mint];
      assert.match(
        String(token.uses ?? ""),
        /^actions\/create-github-app-token@[0-9a-f]{40}$/,
        "the app token does not come from create-github-app-token pinned by SHA",
      );
      assert.deepEqual(token.with, {
        "client-id": gh("vars.RELEASE_APP_CLIENT_ID"),
        "private-key": gh("secrets.RELEASE_APP_PRIVATE_KEY"),
        "permission-contents": "write",
      });
      const checkout = stepAt(
        ctx,
        "commit-back",
        usesAction("actions/checkout"),
      );
      assert.ok(mint < checkout, "the token is minted after the checkout");
      assert.equal(steps[checkout].with?.token, APP_TOKEN);
      const step = steps[stepAt(ctx, "commit-back", runs("commit-back.mjs"))];
      assert.equal(
        step.env?.GH_TOKEN,
        APP_TOKEN,
        "the push is not the release app's, so the bypass does not apply",
      );
      assert.doesNotMatch(
        step.run,
        /--no-push\b/,
        "commit-back.mjs does not push",
      );
      for (const other of steps) {
        assert.doesNotMatch(
          JSON.stringify(other),
          /github\.token/,
          "a step of commit-back uses github.token",
        );
        assert.doesNotMatch(
          String(other.run ?? ""),
          /\bgh pr\b|\bpulls\b/,
          "a step opens a pull request instead of pushing",
        );
      }
      assert.equal(
        job(ctx, "commit-back").permissions,
        undefined,
        "commit-back's github.token is more than read-only",
      );
      for (const id of JOBS.filter((id) => id !== "commit-back"))
        assert.doesNotMatch(
          JSON.stringify(job(ctx, id)),
          /create-github-app-token|RELEASE_APP_|outputs\.token/,
          `${id} mints or reads the release app's token`,
        );
    },
    mutants: {
      "pushes with github.token": (ctx) =>
        ctx.doc.setIn(
          [
            "jobs",
            "commit-back",
            "steps",
            stepAt(ctx, "commit-back", runs("commit-back.mjs")),
            "env",
            "GH_TOKEN",
          ],
          gh("github.token"),
        ),
      "checks out with github.token": (ctx) =>
        ctx.doc.setIn(
          [
            "jobs",
            "commit-back",
            "steps",
            stepAt(ctx, "commit-back", usesAction("actions/checkout")),
            "with",
            "token",
          ],
          gh("github.token"),
        ),
      "unpins the action": (ctx) =>
        ctx.doc.setIn(
          [...APP_PATH(ctx), "uses"],
          "actions/create-github-app-token@v3",
        ),
      "reads the client id from a secret": (ctx) =>
        ctx.doc.setIn(
          [...APP_PATH(ctx), "with", "client-id"],
          gh("secrets.RELEASE_APP_CLIENT_ID"),
        ),
      "uses the deprecated app-id": (ctx) => {
        ctx.doc.deleteIn([...APP_PATH(ctx), "with", "client-id"]);
        ctx.doc.setIn(
          [...APP_PATH(ctx), "with", "app-id"],
          gh("vars.RELEASE_APP_ID"),
        );
      },
      "mints a token with every permission of the app": (ctx) =>
        ctx.doc.deleteIn([...APP_PATH(ctx), "with", "permission-contents"]),
      "mints the token after the checkout": (ctx) => {
        const steps = stepsOf(ctx, "commit-back");
        const mint = steps.findIndex((step) => step.id === APP_STEP);
        const list = ctx.doc.getIn(
          ["jobs", "commit-back", "steps"],
          true,
        ).items;
        const [step] = list.splice(mint, 1);
        list.push(step);
      },
      "grants commit-back contents: write": (ctx) =>
        ctx.doc.setIn(["jobs", "commit-back", "permissions"], {
          contents: "write",
        }),
      "publish mints the app token too": (ctx) =>
        ctx.doc.getIn(["jobs", "publish", "steps"], true).items.unshift(
          ctx.doc.createNode({
            id: APP_STEP,
            uses: stepsOf(ctx, "commit-back").find((s) => s.id === APP_STEP)
              .uses,
          }),
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
        "Always allow",
        "Settings → Secrets and variables → Actions",
        "`RELEASE_APP_CLIENT_ID`",
        "Client ID",
        "`RELEASE_APP_PRIVATE_KEY`",
        "Generate a private key",
        "only this app",
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
      "names no private key": (ctx) => {
        ctx.runbook = ctx.runbook.replace(
          section(ctx.runbook),
          section(ctx.runbook).replaceAll("RELEASE_APP_PRIVATE_KEY", "KEY"),
        );
      },
      "says the bypass reaches every workflow token": (ctx) => {
        ctx.runbook = ctx.runbook.replace(
          section(ctx.runbook),
          section(ctx.runbook).replaceAll("only this app", "every workflow"),
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

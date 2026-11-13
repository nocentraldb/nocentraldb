// Asserts the alpha-npm job of .github/workflows/release.yml (NOC-281;
// docs/release-management.md, "Alpha"): it runs only after the alpha job,
// when that job got as far as an alpha (not skipped by its guard or by
// next-version); it holds id-token: write and contents: read only, and no
// npm token; it checks out the alpha's commit without keeping a token,
// writes the alpha's version, builds, and then publishes through
// scripts/release/npm-publish.mjs under the dist-tag alpha, which is the only
// way it publishes; a dispatch only reports; alpha publishes run one at a
// time and are never cancelled.
//
// The workflow is parsed with the `yaml` package and checked by structure.
// Every check is also run against mutants of the file and must reject each
// one (scripts/lib/workflow-checks.mjs). The helpers these files share are in
// scripts/lib/release-workflow.mjs.
import assert from "node:assert/strict";
import {
  ALPHA,
  clone,
  editRun,
  gh,
  job,
  load,
  runs,
  stepAt,
  stepsOf,
  usesAction,
} from "./lib/release-workflow.mjs";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const NPM = ["jobs", "alpha-npm"];
const npm = (ctx) => job(ctx, "alpha-npm");
const npmSteps = (ctx) => stepsOf(ctx, "alpha-npm");
const at = (ctx, pred) => stepAt(ctx, "alpha-npm", pred);

const OUTPUTS = {
  publish: gh(
    "steps.guard.outputs.skip != 'true' && steps.next.outputs.skip != 'true'",
  ),
  tag: gh("steps.next.outputs.tag"),
  sha: gh("env.SHA"),
};
const PERMISSIONS = { contents: "read", "id-token": "write" };
const CONCURRENCY = { group: "release-alpha-npm", "cancel-in-progress": false };
const PUBLISH = `node scripts/release/npm-publish.mjs --version "\${TAG#v}" --dist-tag alpha $dry`;

const checks = [
  {
    name: "runs after the alpha job, only when it got as far as an alpha",
    check: (ctx) => {
      assert.equal(npm(ctx).needs, "alpha");
      assert.equal(
        String(npm(ctx).if ?? "").trim(),
        "needs.alpha.outputs.publish == 'true'",
      );
      assert.deepEqual(job(ctx, "alpha").outputs, OUTPUTS);
    },
    mutants: {
      "drops needs": (ctx) => ctx.doc.deleteIn([...NPM, "needs"]),
      "drops the if": (ctx) => ctx.doc.deleteIn([...NPM, "if"]),
      "publishes when the guard skipped": (ctx) =>
        ctx.doc.setIn(
          [...ALPHA, "outputs", "publish"],
          gh("steps.next.outputs.skip != 'true'"),
        ),
      "publishes when next-version skipped": (ctx) =>
        ctx.doc.setIn(
          [...ALPHA, "outputs", "publish"],
          gh("steps.guard.outputs.skip != 'true'"),
        ),
      "takes the sha from github.sha": (ctx) =>
        ctx.doc.setIn([...ALPHA, "outputs", "sha"], gh("github.sha")),
    },
  },
  {
    name: "holds id-token: write and contents: read only, and no npm token",
    check: (ctx) => {
      assert.deepEqual(npm(ctx).permissions, PERMISSIONS);
      const text = JSON.stringify(npm(ctx));
      assert.doesNotMatch(text, /secrets\./, "the job reads a secret");
      assert.doesNotMatch(
        text,
        /NODE_AUTH_TOKEN|NPM_TOKEN|_authToken/,
        "the job hands npm a token",
      );
    },
    mutants: {
      "grants contents: write": (ctx) =>
        ctx.doc.setIn([...NPM, "permissions", "contents"], "write"),
      "drops id-token": (ctx) =>
        ctx.doc.deleteIn([...NPM, "permissions", "id-token"]),
      "inherits the workflow's permissions": (ctx) =>
        ctx.doc.deleteIn([...NPM, "permissions"]),
      "hands npm a token": (ctx) =>
        ctx.doc.setIn(
          [
            ...NPM,
            "steps",
            at(ctx, runs("npm-publish.mjs")),
            "env",
            "NODE_AUTH_TOKEN",
          ],
          gh("secrets.NPM_TOKEN"),
        ),
    },
  },
  {
    name: "checks out the alpha's commit and keeps no token in the checkout",
    check: (ctx) => {
      assert.equal(npm(ctx).env?.SHA, gh("needs.alpha.outputs.sha"));
      assert.equal(npm(ctx).env?.TAG, gh("needs.alpha.outputs.tag"));
      const checkout = npmSteps(ctx)[at(ctx, usesAction("actions/checkout"))];
      assert.equal(checkout.with?.ref, gh("env.SHA"));
      assert.equal(checkout.with?.["persist-credentials"], false);
    },
    mutants: {
      "checks out github.sha": (ctx) =>
        ctx.doc.setIn(
          [
            ...NPM,
            "steps",
            at(ctx, usesAction("actions/checkout")),
            "with",
            "ref",
          ],
          gh("github.sha"),
        ),
      "keeps the token": (ctx) =>
        ctx.doc.setIn(
          [
            ...NPM,
            "steps",
            at(ctx, usesAction("actions/checkout")),
            "with",
            "persist-credentials",
          ],
          true,
        ),
      "takes the tag from the ref": (ctx) =>
        ctx.doc.setIn([...NPM, "env", "TAG"], gh("github.ref_name")),
    },
  },
  {
    name: "writes the alpha's version, then builds, then publishes",
    check: (ctx) => {
      const version = at(ctx, runs("set-version.mjs"));
      assert.match(
        npmSteps(ctx)[version].run,
        /node scripts\/release\/set-version\.mjs "\$\{TAG#v\}"/,
      );
      const bindgen = at(ctx, runs("moon run repo:wasm-runner"));
      const build = at(ctx, runs("pnpm -r --if-present run build"));
      const publish = at(ctx, runs("npm-publish.mjs"));
      assert.ok(version < build, "the build runs before the version is set");
      assert.ok(bindgen < build, "the build runs before wasm-bindgen exists");
      assert.ok(build < publish, "publishes before the build");
    },
    mutants: {
      "publishes before the build": (ctx) => {
        const list = ctx.doc.getIn([...NPM, "steps"], true).items;
        const [step] = list.splice(at(ctx, runs("npm-publish.mjs")), 1);
        list.splice(at(ctx, runs("pnpm -r --if-present run build")), 0, step);
      },
      "sets a fixed version": (ctx) =>
        editRun(ctx, "alpha-npm", "set-version.mjs", `"\${TAG#v}"`, "0.1.0"),
    },
  },
  {
    name: "publishes through npm-publish under the dist-tag alpha, and a dispatch only reports",
    check: (ctx) => {
      assert.equal(
        npm(ctx).env?.DRY_RUN,
        gh("github.event_name == 'workflow_dispatch'"),
      );
      const run = String(npmSteps(ctx)[at(ctx, runs("npm-publish.mjs"))].run);
      assert.ok(run.includes(PUBLISH), run);
      assert.match(
        run,
        /if \[ "\$DRY_RUN" = true \]; then dry=--dry-run; else dry=; fi/,
      );
      for (const step of npmSteps(ctx))
        assert.doesNotMatch(
          String(step.run ?? ""),
          /\bnpm publish\b|pnpm publish/,
          "a step publishes around npm-publish",
        );
    },
    mutants: {
      "hard-codes DRY_RUN to false": (ctx) =>
        ctx.doc.setIn([...NPM, "env", "DRY_RUN"], "false"),
      "never passes --dry-run": (ctx) =>
        editRun(
          ctx,
          "alpha-npm",
          "npm-publish.mjs",
          "then dry=--dry-run",
          "then dry=",
        ),
      "publishes under latest": (ctx) =>
        editRun(
          ctx,
          "alpha-npm",
          "npm-publish.mjs",
          "--dist-tag alpha",
          "--dist-tag latest",
        ),
      "publishes with npm directly": (ctx) =>
        ctx.doc.addIn([...NPM, "steps"], {
          run: "cd packages/sdk && npm publish --tag alpha",
        }),
    },
  },
  {
    name: "alpha publishes run one at a time and are never cancelled",
    check: (ctx) => assert.deepEqual(npm(ctx).concurrency, CONCURRENCY),
    mutants: {
      "cancels in progress": (ctx) =>
        ctx.doc.setIn([...NPM, "concurrency", "cancel-in-progress"], true),
      "drops the concurrency": (ctx) =>
        ctx.doc.deleteIn([...NPM, "concurrency"]),
    },
  },
];

runWorkflowChecks({ files: load(), clone, checks });

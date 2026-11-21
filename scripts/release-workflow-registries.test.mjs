// Asserts the registry jobs of the publish side of
// .github/workflows/release.yml (NOC-374; docs/release-management.md, "Beta"
// and "Release"): publish-crates and publish-npm run after the GitHub release
// of a beta or release tag and never on a dispatch; each holds id-token:
// write and contents: read only, and reads no registry token from a secret;
// each checks out the tagged commit without keeping a token and writes the
// tag's version into it. publish-crates verifies every crate with
// crates-publish --verify-only before it fetches a crates.io token through
// Trusted Publishing (rust-lang/crates-io-auth-action), and only then
// publishes through crates-publish, which is the only way it publishes.
// publish-npm builds the packages and stages them through npm-publish, under
// beta for a beta and latest for a release, which is the only way it sends
// them to npm.
//
// The workflow is parsed with the `yaml` package and checked by structure.
// Every check is also run against mutants of the file and must reject each
// one (scripts/lib/workflow-checks.mjs). The helpers these files share are in
// scripts/lib/release-workflow.mjs.
import assert from "node:assert/strict";
import {
  clone,
  editRun,
  gh,
  job,
  load,
  moveStepOf,
  runs,
  stepAt,
  stepsOf,
  usesAction,
} from "./lib/release-workflow.mjs";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const REGISTRIES = ["publish-crates", "publish-npm"];
const CRATES = ["jobs", "publish-crates"];
const NPM = ["jobs", "publish-npm"];
const NOT_DRY_RUN = "needs.verify.outputs.dry_run != 'true'";
const PERMISSIONS = { contents: "read", "id-token": "write" };
const AUTH = "rust-lang/crates-io-auth-action";
const isAuth = usesAction(AUTH);
const VERIFY = `node scripts/release/crates-publish.mjs --version "\${TAG#v}" --verify-only`;
const PUBLISH_CRATES = `node scripts/release/crates-publish.mjs --version "\${TAG#v}"`;
const DIST_TAG = `if [ "$STAGE" = beta ]; then tag=beta; else tag=latest; fi`;
const STAGE_NPM = `node scripts/release/npm-publish.mjs --version "\${TAG#v}" --dist-tag "$tag" --repo "$GITHUB_REPOSITORY"`;
/** A step that sends something to a registry by itself, around the scripts. */
const DIRECT = /\b(?:npm|pnpm|cargo) (?:stage )?publish\b/;

const crates = (ctx) => stepsOf(ctx, "publish-crates");
const at = (ctx, id, pred) => stepAt(ctx, id, pred);
const verifyStep = (ctx) =>
  at(ctx, "publish-crates", (s) =>
    String(s.run ?? "").includes("--verify-only"),
  );
const publishStep = (ctx) =>
  at(
    ctx,
    "publish-crates",
    (s) =>
      String(s.run ?? "").includes("crates-publish.mjs") &&
      !String(s.run).includes("--verify-only"),
  );

const checks = [
  {
    name: "both run after the GitHub release of a beta or release tag, never on a dispatch",
    check: (ctx) => {
      for (const id of REGISTRIES) {
        assert.deepEqual(job(ctx, id).needs, ["verify", "publish"], id);
        assert.ok(
          String(job(ctx, id).if ?? "").includes(NOT_DRY_RUN),
          `${id} runs on a dry run`,
        );
      }
    },
    mutants: {
      "publish-crates before the GitHub release": (ctx) =>
        ctx.doc.setIn([...CRATES, "needs"], ["verify", "build"]),
      "publish-npm on a dry run": (ctx) => ctx.doc.deleteIn([...NPM, "if"]),
    },
  },
  {
    name: "both hold id-token: write and contents: read only, and read no registry token from a secret",
    check: (ctx) => {
      for (const id of REGISTRIES) {
        assert.deepEqual(job(ctx, id).permissions, PERMISSIONS, id);
        const text = JSON.stringify(job(ctx, id));
        assert.doesNotMatch(text, /secrets\./, `${id} reads a secret`);
        assert.doesNotMatch(
          text,
          /NODE_AUTH_TOKEN|NPM_TOKEN|_authToken/,
          `${id} hands npm a token`,
        );
      }
      assert.doesNotMatch(
        JSON.stringify(job(ctx, "publish-npm")),
        /CARGO_REGISTRY_TOKEN|crates-io-auth-action/,
        "publish-npm holds a crates.io token",
      );
      // The crates.io token reaches the publish step only, from the auth step.
      const steps = crates(ctx);
      const publish = publishStep(ctx);
      const auth = at(ctx, "publish-crates", isAuth);
      assert.equal(steps[auth].id, "crates-io-auth");
      assert.equal(steps[auth].with, undefined, "the token is not crates.io's");
      for (const [i, step] of steps.entries())
        assert.equal(
          step.env?.CARGO_REGISTRY_TOKEN,
          i === publish ? gh("steps.crates-io-auth.outputs.token") : undefined,
          `step ${i} of publish-crates`,
        );
      assert.equal(
        job(ctx, "publish-crates").env?.CARGO_REGISTRY_TOKEN,
        undefined,
      );
    },
    mutants: {
      "grants publish-crates contents: write": (ctx) =>
        ctx.doc.setIn([...CRATES, "permissions", "contents"], "write"),
      "inherits the workflow's permissions": (ctx) =>
        ctx.doc.deleteIn([...NPM, "permissions"]),
      "reads the crates.io token from a secret": (ctx) =>
        ctx.doc.setIn(
          [...CRATES, "steps", publishStep(ctx), "env", "CARGO_REGISTRY_TOKEN"],
          gh("secrets.CARGO_REGISTRY_TOKEN"),
        ),
      "hands the token to the verification": (ctx) =>
        ctx.doc.setIn([...CRATES, "steps", verifyStep(ctx), "env"], {
          CARGO_REGISTRY_TOKEN: gh("steps.crates-io-auth.outputs.token"),
        }),
      "hands the token to the whole job": (ctx) =>
        ctx.doc.setIn(
          [...CRATES, "env", "CARGO_REGISTRY_TOKEN"],
          gh("steps.crates-io-auth.outputs.token"),
        ),
      "hands npm a token": (ctx) =>
        ctx.doc.setIn(
          [...NPM, "env", "NODE_AUTH_TOKEN"],
          gh("secrets.NPM_TOKEN"),
        ),
    },
  },
  {
    name: "both check out the tagged commit without keeping a token, and write the tag's version first",
    check: (ctx) => {
      for (const id of REGISTRIES) {
        assert.equal(job(ctx, id).env?.TAG, gh("needs.verify.outputs.tag"), id);
        const steps = stepsOf(ctx, id);
        const checkout = steps[at(ctx, id, usesAction("actions/checkout"))];
        assert.equal(checkout.with?.["persist-credentials"], false, id);
        assert.equal(checkout.with?.ref, undefined, `${id} checks out a ref`);
        const set = at(ctx, id, runs("set-version.mjs"));
        assert.match(
          steps[set].run,
          /node scripts\/release\/set-version\.mjs "\$\{TAG#v\}"/,
        );
        const sends = at(ctx, id, (s) =>
          /crates-publish\.mjs|npm-publish\.mjs/.test(String(s.run ?? "")),
        );
        assert.ok(set < sends, `${id} sends before it sets the version`);
      }
    },
    mutants: {
      "keeps the token in the checkout": (ctx) =>
        ctx.doc.setIn(
          [
            ...NPM,
            "steps",
            at(ctx, "publish-npm", usesAction("actions/checkout")),
            "with",
            "persist-credentials",
          ],
          true,
        ),
      "takes the tag from the ref": (ctx) =>
        ctx.doc.setIn([...CRATES, "env", "TAG"], gh("github.ref_name")),
      "drops set-version": (ctx) =>
        ctx.doc.deleteIn([
          ...CRATES,
          "steps",
          at(ctx, "publish-crates", runs("set-version.mjs")),
        ]),
    },
  },
  {
    name: "publish-crates verifies every crate before it fetches a crates.io token, then publishes through crates-publish only",
    check: (ctx) => {
      const steps = crates(ctx);
      const verify = verifyStep(ctx);
      const auth = at(ctx, "publish-crates", isAuth);
      const publish = publishStep(ctx);
      assert.equal(String(steps[verify].run).trim(), VERIFY);
      assert.equal(String(steps[publish].run).trim(), PUBLISH_CRATES);
      assert.ok(verify < auth, "the token is fetched before the verification");
      assert.ok(auth < publish, "publishes before the token is fetched");
      assert.equal(
        steps.filter(isAuth).length,
        1,
        "more than one crates.io token",
      );
      for (const step of steps)
        assert.doesNotMatch(
          String(step.run ?? ""),
          DIRECT,
          "a step publishes around crates-publish",
        );
    },
    mutants: {
      "fetches the token before the verification": (ctx) =>
        moveStepOf(
          ctx,
          "publish-crates",
          at(ctx, "publish-crates", isAuth),
          verifyStep(ctx),
        ),
      "drops the verification": (ctx) =>
        ctx.doc.deleteIn([...CRATES, "steps", verifyStep(ctx)]),
      "publishes with --verify-only": (ctx) =>
        editRun(
          ctx,
          "publish-crates",
          "crates-publish.mjs",
          "--verify-only",
          "",
        ),
      "publishes with cargo directly": (ctx) =>
        ctx.doc.addIn([...CRATES, "steps"], {
          run: "cargo publish --workspace",
        }),
      "fetches a second token": (ctx) =>
        ctx.doc.addIn([...CRATES, "steps"], {
          uses: crates(ctx)[at(ctx, "publish-crates", isAuth)].uses,
        }),
    },
  },
  {
    name: "publish-npm builds, then stages through npm-publish, under beta for a beta and latest for a release",
    check: (ctx) => {
      const steps = stepsOf(ctx, "publish-npm");
      assert.equal(
        job(ctx, "publish-npm").env?.STAGE,
        gh("needs.verify.outputs.stage"),
      );
      const bindgen = at(ctx, "publish-npm", runs("moon run repo:wasm-runner"));
      const build = at(
        ctx,
        "publish-npm",
        runs("pnpm -r --if-present run build"),
      );
      const send = at(ctx, "publish-npm", runs("npm-publish.mjs"));
      assert.ok(bindgen < build, "the build runs before wasm-bindgen exists");
      assert.ok(build < send, "stages before the build");
      const run = String(steps[send].run);
      assert.ok(run.includes(DIST_TAG), run);
      assert.ok(run.includes(STAGE_NPM), run);
      assert.ok(run.indexOf(DIST_TAG) < run.indexOf(STAGE_NPM), run);
      assert.doesNotMatch(run, /--dry-run/);
      for (const step of steps)
        assert.doesNotMatch(
          String(step.run ?? ""),
          DIRECT,
          "a step sends a package around npm-publish",
        );
    },
    mutants: {
      "stages a beta under latest": (ctx) =>
        editRun(
          ctx,
          "publish-npm",
          "npm-publish.mjs",
          "tag=beta;",
          "tag=latest;",
        ),
      "stages a release under beta": (ctx) =>
        editRun(
          ctx,
          "publish-npm",
          "npm-publish.mjs",
          "else tag=latest",
          "else tag=beta",
        ),
      "stages before the build": (ctx) =>
        moveStepOf(
          ctx,
          "publish-npm",
          at(ctx, "publish-npm", runs("npm-publish.mjs")),
          at(ctx, "publish-npm", runs("pnpm -r --if-present run build")),
        ),
      "stages only a dry run": (ctx) =>
        editRun(
          ctx,
          "publish-npm",
          "npm-publish.mjs",
          '"$GITHUB_REPOSITORY"',
          '"$GITHUB_REPOSITORY" --dry-run',
        ),
      "stages with npm directly": (ctx) =>
        ctx.doc.addIn([...NPM, "steps"], {
          run: "cd packages/sdk && npm stage publish --tag beta",
        }),
      "reads the stage from the ref": (ctx) =>
        ctx.doc.setIn([...NPM, "env", "STAGE"], "beta"),
    },
  },
];

runWorkflowChecks({ files: load(), clone, checks });

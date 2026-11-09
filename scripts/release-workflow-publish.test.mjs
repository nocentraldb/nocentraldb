// Asserts the publish side of .github/workflows/release.yml (epic mt08.q8rt
// §11, docs/release-management.md "Beta" and "Release"): verify, build,
// publish and commit-back run for v* tags that are not alphas, or for a
// dispatch with a tag, which is a dry run that never reaches publish or
// commit-back; verify fetches the release branches, runs verify-tag on the
// commit and then the full gate, before build starts; build writes the tag's
// version into its own checkout before it builds; publish makes an idempotent
// GitHub release (a prerelease for a beta, latest for a release) and only
// reports on the registries; commit-back checks out main and leaves the
// commit to commit-back.mjs; no job that holds contents: write leaves its
// token in the checkout; the jobs of the publish side share one concurrency
// group per ref that never cancels a running job.
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
  normalise,
  runs,
  stepAt,
  stepsOf,
  usesAction,
} from "./lib/release-workflow.mjs";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const PUBLISH_IF = [
  "(github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v') && !contains(github.ref_name, '-alpha.'))",
  "(github.event_name == 'workflow_dispatch' && inputs.tag != '')",
].join(" || ");
const NOT_DRY_RUN = "needs.verify.outputs.dry_run != 'true'";
const PUBLISH_CONCURRENCY = {
  group: `release-publish-${gh("github.ref")}`,
  "cancel-in-progress": false,
};
const PUBLISH_SIDE = ["verify", "build", "publish", "commit-back"];

const checks = [
  {
    name: "the publish side runs for v* tags that are not alphas, or for a dispatch with a tag",
    check: (ctx) => {
      assert.equal(normalise(job(ctx, "verify").if), PUBLISH_IF);
      // Every other job of the publish side follows verify.
      for (const id of ["build", "publish", "commit-back"])
        assert.ok(
          [job(ctx, id).needs].flat().includes("verify"),
          `${id} does not need verify`,
        );
    },
    mutants: {
      "drops the alpha exclusion": (ctx) =>
        ctx.doc.setIn(
          ["jobs", "verify", "if"],
          PUBLISH_IF.replace(" && !contains(github.ref_name, '-alpha.')", ""),
        ),
      "drops the tag prefix": (ctx) =>
        ctx.doc.setIn(
          ["jobs", "verify", "if"],
          PUBLISH_IF.replace(" && startsWith(github.ref, 'refs/tags/v')", ""),
        ),
      "dispatch clause with inputs.tag == ''": (ctx) =>
        ctx.doc.setIn(
          ["jobs", "verify", "if"],
          PUBLISH_IF.replace("inputs.tag != ''", "inputs.tag == ''"),
        ),
      "drops the if": (ctx) => ctx.doc.deleteIn(["jobs", "verify", "if"]),
    },
  },
  {
    name: "a dispatch never publishes: publish waits for a non-dry run and commit-back for publish",
    check: (ctx) => {
      assert.equal(
        job(ctx, "verify").outputs?.dry_run,
        gh("github.event_name == 'workflow_dispatch'"),
      );
      assert.ok(
        String(job(ctx, "publish").if ?? "").includes(NOT_DRY_RUN),
        "publish runs on a dry run",
      );
      assert.ok(
        [job(ctx, "commit-back").needs].flat().includes("publish"),
        "commit-back does not wait for publish",
      );
    },
    mutants: {
      "drops the if of publish": (ctx) =>
        ctx.doc.deleteIn(["jobs", "publish", "if"]),
      "hard-codes dry_run": (ctx) =>
        ctx.doc.setIn(["jobs", "verify", "outputs", "dry_run"], "false"),
      "commit-back needs only verify": (ctx) =>
        ctx.doc.setIn(["jobs", "commit-back", "needs"], ["verify"]),
    },
  },
  {
    name: "verify checks the tag on the commit before the full gate, and before build starts",
    check: (ctx) => {
      const verify = stepsOf(ctx, "verify");
      const v = stepAt(ctx, "verify", (step) => step.id === "verify");
      const gates = stepAt(ctx, "verify", (step) => step.run === "pnpm gates");
      assert.ok(v < gates, "the gate runs before verify-tag");
      assert.match(
        verify[v].run,
        /node scripts\/release\/verify-tag\.mjs --tag "\$TAG" --commit "\$GITHUB_SHA" --json \| tee verify\.json/,
      );
      // One source for the tag: the job's env, read by the step and the output.
      assert.equal(
        job(ctx, "verify").env?.TAG,
        gh("inputs.tag || github.ref_name"),
      );
      assert.equal(verify[v].env?.TAG, undefined);
      for (const [output, step] of [
        ["stage", "stage"],
        ["version", "version"],
        ["last_beta", "last_beta"],
      ])
        assert.equal(
          job(ctx, "verify").outputs?.[output],
          gh(`steps.verify.outputs.${step}`),
        );
      assert.equal(job(ctx, "verify").outputs?.tag, gh("env.TAG"));
      assert.ok(
        verify[v].run.includes(
          "last_beta=$(jq -r '.lastBeta // \"none\"' verify.json)",
        ),
        "the last beta does not fall back to none, the value render-notes takes",
      );
      assert.equal(job(ctx, "build").needs, "verify");
      assert.deepEqual(job(ctx, "publish").needs, ["verify", "build"]);
    },
    mutants: {
      "runs the gate before verify-tag": (ctx) =>
        moveStepOf(
          ctx,
          "verify",
          stepAt(ctx, "verify", (step) => step.run === "pnpm gates"),
          0,
        ),
      "drops the gate": (ctx) =>
        ctx.doc.deleteIn([
          "jobs",
          "verify",
          "steps",
          stepAt(ctx, "verify", (step) => step.run === "pnpm gates"),
        ]),
      "sets TAG on the step again": (ctx) =>
        ctx.doc.setIn(
          [
            "jobs",
            "verify",
            "steps",
            stepAt(ctx, "verify", (step) => step.id === "verify"),
            "env",
          ],
          { TAG: gh("inputs.tag || github.ref_name") },
        ),
      "outputs the tag from github.ref_name": (ctx) =>
        ctx.doc.setIn(
          ["jobs", "verify", "outputs", "tag"],
          gh("github.ref_name"),
        ),
      "verifies github.ref instead of the commit": (ctx) =>
        editRun(ctx, "verify", "verify-tag.mjs", '"$GITHUB_SHA"', '"$TAG"'),
      "build without needs": (ctx) =>
        ctx.doc.deleteIn(["jobs", "build", "needs"]),
      "publish needs verify only": (ctx) =>
        ctx.doc.setIn(["jobs", "publish", "needs"], ["verify"]),
      "falls back to an empty last beta": (ctx) =>
        editRun(ctx, "verify", "verify-tag.mjs", '// "none"', '// ""'),
    },
  },
  {
    name: "verify fetches the release branches and every tag before verify-tag runs",
    check: (ctx) => {
      const step = stepsOf(ctx, "verify")[
        stepAt(ctx, "verify", (s) => s.id === "verify")
      ];
      const fetch = step.run.indexOf(
        "git fetch origin '+refs/heads/release/*:refs/remotes/origin/release/*'",
      );
      assert.notEqual(fetch, -1, "the release branches are not fetched");
      assert.ok(fetch < step.run.indexOf("verify-tag.mjs"), "fetched too late");
      const checkout = stepsOf(ctx, "verify")[
        stepAt(ctx, "verify", usesAction("actions/checkout"))
      ];
      assert.equal(checkout.with?.["fetch-depth"], 0);
      assert.equal(checkout.with?.["fetch-tags"], true);
    },
    mutants: {
      "drops the fetch": (ctx) =>
        editRun(
          ctx,
          "verify",
          "verify-tag.mjs",
          "git fetch origin '+refs/heads/release/*:refs/remotes/origin/release/*'\n",
          "",
        ),
      "fetches depth 1": (ctx) =>
        ctx.doc.setIn(
          [
            "jobs",
            "verify",
            "steps",
            stepAt(ctx, "verify", usesAction("actions/checkout")),
            "with",
            "fetch-depth",
          ],
          1,
        ),
    },
  },
  {
    name: "build writes the version from the tag into its own checkout before it builds",
    check: (ctx) => {
      const set = stepAt(ctx, "build", runs("set-version.mjs"));
      const step = stepsOf(ctx, "build")[set];
      assert.match(
        step.run,
        /node scripts\/release\/set-version\.mjs "\$\{TAG#v\}"/,
      );
      assert.equal(step.env?.TAG, gh("needs.verify.outputs.tag"));
      assert.ok(
        set < stepAt(ctx, "build", runs("cargo build --release --locked")),
      );
      assert.ok(
        set < stepAt(ctx, "build", runs("pnpm -r --if-present run build")),
      );
    },
    mutants: {
      "drops set-version": (ctx) =>
        ctx.doc.deleteIn([
          "jobs",
          "build",
          "steps",
          stepAt(ctx, "build", runs("set-version.mjs")),
        ]),
      "sets the version after cargo build": (ctx) =>
        moveStepOf(
          ctx,
          "build",
          stepAt(ctx, "build", runs("set-version.mjs")),
          stepsOf(ctx, "build").length - 1,
        ),
      "builds without --locked": (ctx) =>
        editRun(ctx, "build", "cargo build", " --locked", ""),
    },
  },
  {
    name: "the GitHub release is a prerelease for a beta and latest for a release, and idempotent",
    check: (ctx) => {
      const step = stepsOf(ctx, "publish")[
        stepAt(ctx, "publish", runs("gh release create"))
      ];
      const view = step.run.indexOf('gh release view "$TAG"');
      assert.notEqual(view, -1, "no check for an existing release");
      assert.ok(
        view < step.run.indexOf("gh release create"),
        "check after create",
      );
      assert.match(
        step.run,
        /gh release create "\$TAG" --prerelease --verify-tag --title "\$TAG" --notes-file notes\.md/,
      );
      assert.match(
        step.run,
        /gh release create "\$TAG" --latest --verify-tag --title "\$TAG" --notes-file notes\.md/,
      );
      assert.match(
        step.run,
        /if \[ "\$STAGE" = beta \]|elif \[ "\$STAGE" = beta \]/,
      );
      assert.equal(step.env?.GH_TOKEN, gh("github.token"));
      assert.equal(step.env?.GH_REPO, gh("github.repository"));
      assert.equal(step.env?.TAG, gh("needs.verify.outputs.tag"));
      assert.equal(step.env?.STAGE, gh("needs.verify.outputs.stage"));
      const notes = stepsOf(ctx, "publish")[
        stepAt(ctx, "publish", runs("render-notes.mjs"))
      ];
      assert.match(
        notes.run,
        /--beta "\$VERSION" --since "\$LAST_BETA" > notes\.md/,
      );
      assert.match(notes.run, /--release "\$VERSION" > notes\.md/);
      assert.equal(notes.env?.LAST_BETA, gh("needs.verify.outputs.last_beta"));
    },
    mutants: {
      "drops --prerelease": (ctx) =>
        editRun(ctx, "publish", "gh release create", " --prerelease", ""),
      "drops the existing-release check": (ctx) =>
        editRun(
          ctx,
          "publish",
          "gh release create",
          'gh release view "$TAG"',
          "false",
        ),
      "drops --latest": (ctx) =>
        editRun(ctx, "publish", "gh release create", " --latest", ""),
      "drops --verify-tag": (ctx) =>
        editRun(ctx, "publish", "gh release create", " --verify-tag", ""),
      "renders beta notes since nothing": (ctx) =>
        editRun(ctx, "publish", "render-notes.mjs", '"$LAST_BETA"', "none"),
    },
  },
  {
    name: "the registry steps only report what publishable says, and publish nothing yet",
    check: (ctx) => {
      for (const registry of ["npm", "crates.io"]) {
        const step = stepsOf(ctx, "publish")[
          stepAt(ctx, "publish", (s) => s.name === registry)
        ];
        assert.match(
          step.run,
          /node scripts\/release\/publishable\.mjs --json/,
        );
        assert.match(step.run, /skip \\\(\.name\): \\\(\.reason\)/);
      }
      for (const step of stepsOf(ctx, "publish"))
        assert.doesNotMatch(
          String(step.run ?? ""),
          /\b(?:npm|pnpm|cargo) publish\b/,
          "a registry is published to",
        );
    },
    mutants: {
      "publishes to npm": (ctx) =>
        ctx.doc.setIn(
          [
            "jobs",
            "publish",
            "steps",
            stepAt(ctx, "publish", (s) => s.name === "npm"),
            "run",
          ],
          "pnpm -r publish --tag beta",
        ),
      "drops the crates.io step": (ctx) =>
        ctx.doc.deleteIn([
          "jobs",
          "publish",
          "steps",
          stepAt(ctx, "publish", (s) => s.name === "crates.io"),
        ]),
    },
  },
  {
    name: "commit-back checks out main and leaves the commit to commit-back.mjs",
    check: (ctx) => {
      const steps = stepsOf(ctx, "commit-back");
      const checkout =
        steps[stepAt(ctx, "commit-back", usesAction("actions/checkout"))];
      assert.equal(checkout.with?.ref, "main");
      assert.equal(checkout.with?.["fetch-depth"], 0);
      assert.equal(checkout.with?.["fetch-tags"], true);
      const step = steps[stepAt(ctx, "commit-back", runs("commit-back.mjs"))];
      assert.match(
        step.run,
        /node scripts\/release\/commit-back\.mjs --tag "\$TAG" --stage "\$STAGE" --version "\$VERSION" --date "\$PUBLISHED"/,
      );
      // The index date is fixed at publishing: the release's own UTC day,
      // never the clock of this job, so a re-run indexes the same day.
      const published = step.run.indexOf(
        `PUBLISHED="$(gh release view "$TAG" --json publishedAt --jq '.publishedAt[:10]')"`,
      );
      assert.notEqual(
        published,
        -1,
        "the date is not the release's publishedAt",
      );
      assert.ok(published < step.run.indexOf("commit-back.mjs"));
      assert.doesNotMatch(
        step.run,
        /\bdate\b -u/,
        "the date is the job's clock",
      );
      assert.equal(step.env?.GH_REPO, gh("github.repository"));
      assert.equal(step.env?.TAG, gh("needs.verify.outputs.tag"));
      assert.equal(step.env?.STAGE, gh("needs.verify.outputs.stage"));
      assert.equal(step.env?.VERSION, gh("needs.verify.outputs.version"));
      for (const other of steps)
        assert.doesNotMatch(
          String(other.run ?? ""),
          /git (?:commit|push)\b/,
          "a step commits or pushes by itself",
        );
    },
    mutants: {
      "drops the ref": (ctx) =>
        ctx.doc.deleteIn([
          "jobs",
          "commit-back",
          "steps",
          stepAt(ctx, "commit-back", usesAction("actions/checkout")),
          "with",
          "ref",
        ]),
      "commits inline instead": (ctx) =>
        editRun(
          ctx,
          "commit-back",
          "commit-back.mjs",
          /node scripts\/release\/commit-back\.mjs[^\n]*/,
          'git commit -am "index" && git push origin HEAD:main',
        ),
      "drops the date": (ctx) =>
        editRun(
          ctx,
          "commit-back",
          "commit-back.mjs",
          ' --date "$PUBLISHED"',
          "",
        ),
      "dates by the job's clock": (ctx) =>
        editRun(
          ctx,
          "commit-back",
          "commit-back.mjs",
          '--date "$PUBLISHED"',
          '--date "$(date -u +%F)"',
        ),
      "reads the date from the clock into PUBLISHED": (ctx) =>
        editRun(
          ctx,
          "commit-back",
          "commit-back.mjs",
          `"$(gh release view "$TAG" --json publishedAt --jq '.publishedAt[:10]')"`,
          '"$(date -u +%F)"',
        ),
    },
  },
  {
    name: "publish and commit-back keep their write token out of the checkout; only the commit-back step gets the release app's token",
    check: (ctx) => {
      for (const id of ["publish", "commit-back"]) {
        const checkout = stepsOf(ctx, id)[
          stepAt(ctx, id, usesAction("actions/checkout"))
        ];
        assert.equal(checkout.with?.["persist-credentials"], false, id);
      }
      const step = stepsOf(ctx, "commit-back")[
        stepAt(ctx, "commit-back", runs("commit-back.mjs"))
      ];
      assert.equal(step.env?.GH_TOKEN, gh("steps.app-token.outputs.token"));
      assert.match(step.run, /GIT_CONFIG_KEY_0=http\.extraHeader/);
    },
    mutants: {
      "persists the publish token": (ctx) =>
        ctx.doc.setIn(
          [
            "jobs",
            "publish",
            "steps",
            stepAt(ctx, "publish", usesAction("actions/checkout")),
            "with",
            "persist-credentials",
          ],
          true,
        ),
      "persists the commit-back token": (ctx) =>
        ctx.doc.deleteIn([
          "jobs",
          "commit-back",
          "steps",
          stepAt(ctx, "commit-back", usesAction("actions/checkout")),
          "with",
          "persist-credentials",
        ]),
    },
  },
  {
    name: "every job of the publish side is in the one group of its ref, which never cancels a running job",
    check: (ctx) => {
      for (const id of PUBLISH_SIDE)
        assert.deepEqual(job(ctx, id).concurrency, PUBLISH_CONCURRENCY, id);
    },
    mutants: {
      "cancels in progress": (ctx) =>
        ctx.doc.setIn(
          ["jobs", "verify", "concurrency", "cancel-in-progress"],
          true,
        ),
      "one group for every ref": (ctx) =>
        ctx.doc.setIn(
          ["jobs", "verify", "concurrency", "group"],
          "release-publish",
        ),
      "build outside the group": (ctx) =>
        ctx.doc.deleteIn(["jobs", "build", "concurrency"]),
      "publish outside the group": (ctx) =>
        ctx.doc.deleteIn(["jobs", "publish", "concurrency"]),
      "commit-back outside the group": (ctx) =>
        ctx.doc.deleteIn(["jobs", "commit-back", "concurrency"]),
      "commit-back in a group of its own": (ctx) =>
        ctx.doc.setIn(
          ["jobs", "commit-back", "concurrency", "group"],
          `release-commit-back-${gh("github.ref")}`,
        ),
    },
  },
];

runWorkflowChecks({ files: load(), clone, checks });

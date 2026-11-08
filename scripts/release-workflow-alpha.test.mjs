// Asserts the alpha job of .github/workflows/release.yml (epic mt08.q8rt
// §11, docs/release-management.md "Alpha"): it runs only for a green ci run
// of a push to this repository's main, or a dispatch without a tag, never for
// a run a pull request (a fork's included) triggered; the head guard is the
// first step, runs before any checkout with the runner's own git, fails
// closed and gates every later step; the checkout is the run's head_sha with
// full history and tags; a dispatch never tags or publishes; the alpha
// honours next-version's skip and publishes a prerelease with the rendered
// notes, creating its tag in the same act, refusing a tag that already exists
// on the remote without a release, with no write token left in the checkout;
// alphas run one at a time and are never cancelled; a dry run off main fails
// at the guard.
//
// The workflow is parsed with the `yaml` package and checked by structure.
// Every check is also run against mutants of the file and must reject each
// one (scripts/lib/workflow-checks.mjs). The helpers these files share are in
// scripts/lib/release-workflow.mjs.
import assert from "node:assert/strict";
import {
  ALPHA,
  alpha,
  at,
  checkoutStep,
  clone,
  editRun,
  GUARDED,
  gh,
  load,
  normalise,
  publishStep,
  runs,
  STEPS,
  steps,
} from "./lib/release-workflow.mjs";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const moveStep = (ctx, from, to) => {
  const list = ctx.doc.getIn(STEPS, true).items;
  const [step] = list.splice(from, 1);
  list.splice(to, 0, step);
};

const RUN_CONDITIONS = [
  "github.event_name == 'workflow_run'",
  "github.event.workflow_run.conclusion == 'success'",
  "github.event.workflow_run.event == 'push'",
  "github.event.workflow_run.head_branch == 'main'",
  "github.event.workflow_run.head_repository.full_name == github.repository",
];
const DISPATCH = "github.event_name == 'workflow_dispatch' && inputs.tag == ''";
const ALPHA_IF = `(${RUN_CONDITIONS.join(" && ")}) || (${DISPATCH})`;
const NOT_SKIPPED = "steps.next.outputs.skip != 'true'";
const NOT_DRY = "env.DRY_RUN != 'true'";
const SHA = gh("github.event.workflow_run.head_sha || github.sha");
const DRY_RUN = gh("github.event_name == 'workflow_dispatch'");
const CHECKOUT_WITH = {
  ref: gh("env.SHA"),
  "fetch-depth": 0,
  "fetch-tags": true,
  "persist-credentials": false,
};
const CONCURRENCY = { group: "release-alpha", "cancel-in-progress": false };
const OFF_MAIN =
  'if [ "$DRY_RUN" = true ] && [ "$GITHUB_REF" != refs/heads/main ]; then';
const REMOTE_TAG =
  'existing="$(gh api "repos/$GH_REPO/git/matching-refs/tags/$TAG" | jq -r --arg ref "refs/tags/$TAG" \'.[] | select(.ref == $ref) | .ref\')"';

/** Sets the alpha job's `if` to the expected one with `clause` removed. */
const ifWithout = (clause) => (ctx) =>
  ctx.doc.setIn(
    [...ALPHA, "if"],
    ALPHA_IF.replace(` && ${clause}`, "").replace(`${clause} && `, ""),
  );

const checks = [
  {
    name: "the alpha job runs only for a green push run of this repository's main, or a tagless dispatch",
    check: (ctx) => assert.equal(normalise(alpha(ctx).if), ALPHA_IF),
    mutants: {
      "drops the repository condition": ifWithout(RUN_CONDITIONS[4]),
      "drops event == push": ifWithout(RUN_CONDITIONS[2]),
      "drops conclusion == success": ifWithout(RUN_CONDITIONS[1]),
      "drops head_branch == main": ifWithout(RUN_CONDITIONS[3]),
      "dispatch clause without inputs.tag == ''": (ctx) =>
        ctx.doc.setIn(
          [...ALPHA, "if"],
          ALPHA_IF.replace(" && inputs.tag == ''", ""),
        ),
      "drops the if": (ctx) => ctx.doc.deleteIn([...ALPHA, "if"]),
    },
  },
  {
    name: "the head guard is the first step, runs no checked-out code, fails closed, and every later step waits for it",
    check: (ctx) => {
      const [guard, ...rest] = steps(ctx);
      assert.equal(guard?.id, "guard");
      assert.equal(guard.uses, undefined, "the guard uses an action");
      const run = String(guard.run);
      assert.ok(run.includes("git "), "the guard does not run git");
      assert.match(run, /git (?:-c \S+ )*ls-remote --exit-code /);
      assert.match(run, /ls-remote [^\n]*refs\/heads\/main/);
      assert.match(run, /"\$head" != "\$SHA"/);
      assert.equal(guard.env?.GH_TOKEN, gh("github.token"));
      assert.ok(rest.length > 0, "no step after the guard");
      for (const [i, step] of rest.entries())
        assert.ok(
          String(step.if ?? "").includes(GUARDED),
          `step ${i + 1} does not wait for the guard`,
        );
    },
    mutants: {
      "moves the checkout before the guard": (ctx) =>
        moveStep(ctx, checkoutStep(ctx), 0),
      "drops the if of the checkout": (ctx) =>
        ctx.doc.deleteIn([...STEPS, checkoutStep(ctx), "if"]),
      "gives the guard a uses": (ctx) =>
        ctx.doc.setIn([...STEPS, 0, "uses"], "actions/checkout@v7"),
      "lets ls-remote succeed without main": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, 0, "run"],
          String(ctx.doc.getIn([...STEPS, 0, "run"])).replace(
            "ls-remote --exit-code ",
            "ls-remote ",
          ),
        ),
      "asks for another branch": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, 0, "run"],
          String(ctx.doc.getIn([...STEPS, 0, "run"])).replace(
            /(ls-remote [^\n]*)refs\/heads\/main/,
            "$1refs/heads/dev",
          ),
        ),
      "reads anonymously": (ctx) => ctx.doc.deleteIn([...STEPS, 0, "env"]),
    },
  },
  {
    name: "checks out the run's head_sha with full history and tags",
    check: (ctx) => {
      assert.deepEqual(steps(ctx)[checkoutStep(ctx)].with, CHECKOUT_WITH);
      assert.equal(alpha(ctx).env?.SHA, SHA);
    },
    mutants: {
      "drops ref": (ctx) =>
        ctx.doc.deleteIn([...STEPS, checkoutStep(ctx), "with", "ref"]),
      "fetches depth 1": (ctx) =>
        ctx.doc.setIn([...STEPS, checkoutStep(ctx), "with", "fetch-depth"], 1),
      "persists the write token in .git/config": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, checkoutStep(ctx), "with", "persist-credentials"],
          true,
        ),
      "sets SHA to github.sha only": (ctx) =>
        ctx.doc.setIn([...ALPHA, "env", "SHA"], gh("github.sha")),
    },
  },
  {
    name: "never tags or publishes on a dispatch",
    check: (ctx) => {
      assert.equal(alpha(ctx).env?.DRY_RUN, DRY_RUN);
      const writers = steps(ctx).filter(
        (step) =>
          runs("git tag")(step) ||
          runs("git push")(step) ||
          runs("gh release")(step),
      );
      assert.ok(writers.length > 0, "no step tags or publishes");
      for (const step of writers)
        assert.ok(String(step.if ?? "").includes(NOT_DRY), step.run);
    },
    mutants: {
      "drops the dry-run clause": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, publishStep(ctx), "if"],
          String(steps(ctx)[publishStep(ctx)].if).replace(` && ${NOT_DRY}`, ""),
        ),
      "hard-codes DRY_RUN to false": (ctx) =>
        ctx.doc.setIn([...ALPHA, "env", "DRY_RUN"], "false"),
      "adds a push step without the clause": (ctx) =>
        ctx.doc.addIn(STEPS, {
          if: GUARDED,
          run: 'git push origin "refs/tags/$TAG"',
        }),
    },
  },
  {
    name: "publishes next-version's tag as a prerelease with the rendered notes, unless next-version skips",
    check: (ctx) => {
      const step = steps(ctx)[publishStep(ctx)];
      assert.ok(String(step.if).includes(NOT_SKIPPED), "publish ignores skip");
      assert.equal(step.env?.TAG, gh("steps.next.outputs.tag"));
      assert.equal(step.env?.GH_REPO, gh("github.repository"));
      // One act: the API creates the tag on $SHA together with the release,
      // so a failed create leaves no tag that would make a re-run skip.
      assert.match(
        step.run,
        /gh release create "\$TAG" --target "\$SHA" --prerelease --title "\$TAG" --notes-file alpha-notes\.md/,
      );
      // A re-run after a successful create does not create it twice.
      const view = step.run.indexOf('gh release view "$TAG"');
      assert.ok(view !== -1, "no check for an existing release");
      assert.ok(
        view < step.run.indexOf("gh release create"),
        "check after create",
      );
      for (const other of steps(ctx))
        assert.doesNotMatch(
          String(other.run ?? ""),
          /git (?:tag|push)\b/,
          "a step tags or pushes with git: the tag must come with its release",
        );
      const next = steps(ctx)[at(ctx, runs("next-version.mjs"))];
      assert.ok(
        next.run.includes(
          `previous=$(jq -r '.previousAlpha // "none"' next.json)`,
        ),
        "the previous alpha does not fall back to none, the value render-notes takes",
      );
      const notes = steps(ctx)[at(ctx, runs("render-notes.mjs"))];
      assert.ok(String(notes.if).includes(NOT_SKIPPED), "notes ignore skip");
      assert.match(notes.run, /--alpha --since "\$PREVIOUS"/);
      assert.equal(notes.env?.PREVIOUS, gh("steps.next.outputs.previous"));
      assert.equal(steps(ctx)[at(ctx, runs("next-version.mjs"))].id, "next");
    },
    mutants: {
      "ignores the skip": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, publishStep(ctx), "if"],
          String(steps(ctx)[publishStep(ctx)].if).replace(
            ` && ${NOT_SKIPPED}`,
            "",
          ),
        ),
      "tags with git before the release": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, publishStep(ctx), "run"],
          `git tag "$TAG" "$SHA"\ngit push origin "refs/tags/$TAG"\n${steps(ctx)[publishStep(ctx)].run}`,
        ),
      "creates the release without --target": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, publishStep(ctx), "run"],
          steps(ctx)[publishStep(ctx)].run.replace(' --target "$SHA"', ""),
        ),
      "drops the existing-release check": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, publishStep(ctx), "run"],
          'gh release create "$TAG" --target "$SHA" --prerelease --title "$TAG" --notes-file alpha-notes.md\n',
        ),
      "falls back to an empty previous alpha": (ctx) => {
        const i = at(ctx, runs("next-version.mjs"));
        ctx.doc.setIn(
          [...STEPS, i, "run"],
          steps(ctx)[i].run.replace('// "none"', '// ""'),
        );
      },
      "drops GH_REPO": (ctx) =>
        ctx.doc.deleteIn([...STEPS, publishStep(ctx), "env", "GH_REPO"]),
      "publishes a full release": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, publishStep(ctx), "run"],
          steps(ctx)[publishStep(ctx)].run.replace(" --prerelease", ""),
        ),
      "renders the notes since nothing": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, at(ctx, runs("render-notes.mjs")), "env", "PREVIOUS"],
          "none",
        ),
    },
  },
  {
    name: "alphas run one at a time and are never cancelled",
    check: (ctx) => assert.deepEqual(alpha(ctx).concurrency, CONCURRENCY),
    mutants: {
      "cancels in progress": (ctx) =>
        ctx.doc.setIn([...ALPHA, "concurrency", "cancel-in-progress"], true),
      "groups per SHA": (ctx) =>
        ctx.doc.setIn(
          [...ALPHA, "concurrency", "group"],
          `release-alpha-${gh("github.sha")}`,
        ),
    },
  },
  {
    name: "a dry run off main fails at the guard instead of skipping green",
    check: (ctx) => {
      const run = String(steps(ctx)[0]?.run ?? "");
      const refused = run.indexOf(OFF_MAIN);
      assert.notEqual(refused, -1, "the guard does not test a dry run's ref");
      const exit = run.indexOf("exit 1", refused);
      assert.ok(
        exit !== -1 && exit < run.indexOf("fi", refused),
        "a dry run off main does not fail",
      );
      assert.ok(
        refused < run.indexOf("ls-remote"),
        "the dry run's ref is tested after ls-remote",
      );
    },
    mutants: {
      "drops the check": (ctx) =>
        editRun(ctx, "alpha", "ls-remote", OFF_MAIN, "if false; then"),
      "lets it pass": (ctx) =>
        editRun(ctx, "alpha", "ls-remote", /exit 1/, "exit 0"),
      "tests another branch": (ctx) =>
        editRun(
          ctx,
          "alpha",
          "ls-remote",
          "refs/heads/main ]",
          "refs/heads/dev ]",
        ),
    },
  },
  {
    name: "the alpha refuses a tag that already exists on the remote without a release",
    check: (ctx) => {
      const run = String(steps(ctx)[publishStep(ctx)].run);
      const lookup = run.indexOf(REMOTE_TAG);
      assert.notEqual(lookup, -1, "the remote is not asked for the tag");
      assert.ok(
        lookup > run.indexOf('gh release view "$TAG"'),
        "the tag is looked up before the release is",
      );
      const create = run.indexOf("gh release create");
      assert.ok(lookup < create, "the tag is looked up after the create");
      assert.match(
        run.slice(lookup, create),
        /if \[ -n "\$existing" \]; then\n[^\n]*::error::[^\n]*\n\s*exit 1\n/,
      );
    },
    mutants: {
      "drops the lookup": (ctx) =>
        editRun(ctx, "alpha", "gh release create", REMOTE_TAG, 'existing=""'),
      "looks up after the create": (ctx) => {
        editRun(ctx, "alpha", "gh release create", REMOTE_TAG, 'existing=""');
        editRun(
          ctx,
          "alpha",
          "gh release create",
          /(\n\s*)(gh release create [^\n]*)/,
          (_, indent, create) => `${indent}${create}${indent}${REMOTE_TAG}`,
        );
      },
      "only warns": (ctx) =>
        editRun(
          ctx,
          "alpha",
          "gh release create",
          /(::error::[^\n]*\n\s*)exit 1/,
          "$1true",
        ),
    },
  },
];

runWorkflowChecks({ files: load(), clone, checks });

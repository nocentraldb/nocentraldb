// Asserts the invariants of .github/workflows/release.yml (epic mt08.q8rt
// §11, docs/release-management.md "Alpha", "Beta" and "Release"): the
// triggers; read-only by default, contents: write on the jobs alpha, publish
// and commit-back only, verify and build inheriting read; the alpha job runs
// only for a green ci run of a push to this repository's main, or a dispatch
// without a tag, never for a run a pull request (a fork's included)
// triggered; the head guard is the first step, runs before any checkout with
// the runner's own git, fails closed and gates every later step; the
// checkout is the run's head_sha with full history and tags; every step of
// every job runs in bash with pipefail; a dispatch never tags or publishes;
// the alpha honours next-version's skip and publishes a prerelease with the
// rendered notes, creating its tag in the same act, with no write token left
// in the checkout.
//
// The publish side: verify, build, publish and commit-back run for v* tags
// that are not alphas, or for a dispatch with a tag, which is a dry run that
// never reaches publish or commit-back; verify fetches the release branches,
// runs verify-tag on the commit and then the full gate, before build starts;
// build writes the tag's version into its own checkout before it builds;
// publish makes an idempotent GitHub release (a prerelease for a beta, latest
// for a release) and only reports on the registries; commit-back checks out
// main and leaves the commit to commit-back.mjs; no job that holds contents:
// write leaves its token in the checkout. Every job is bounded in time, the
// publish side never cancels a run of the same ref, and every action is
// SHA-pinned.
//
// The workflow is parsed with the `yaml` package and checked by structure.
// Every check is also run against mutants of the file and must reject each
// one (scripts/lib/workflow-checks.mjs).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseDocument } from "yaml";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const load = () => {
  const doc = parseDocument(
    readFileSync(join(root, ".github/workflows/release.yml"), "utf8"),
  );
  assert.deepEqual(doc.errors, [], "release.yml does not parse");
  return { doc };
};
const clone = (ctx) => ({ doc: ctx.doc.clone() });

const workflow = (ctx) => ctx.doc.toJS();
const ALPHA = ["jobs", "alpha"];
const STEPS = [...ALPHA, "steps"];
const alpha = (ctx) => workflow(ctx).jobs?.alpha ?? {};
const steps = (ctx) => alpha(ctx).steps ?? [];
const JOBS = ["alpha", "verify", "build", "publish", "commit-back"];
const job = (ctx, id) => workflow(ctx).jobs?.[id] ?? {};
const stepsOf = (ctx, id) => job(ctx, id).steps ?? [];
/** The index of the first step of job `id` that satisfies `pred`. */
const stepAt = (ctx, id, pred) => {
  const index = stepsOf(ctx, id).findIndex(pred);
  assert.notEqual(index, -1, `no step of ${id} matches`);
  return index;
};
const usesAction = (name) => (step) =>
  String(step.uses ?? "").startsWith(`${name}@`);
/** Replaces `from` by `to` in the run of the first step of `id` that runs `text`. */
const editRun = (ctx, id, text, from, to) => {
  const i = stepAt(ctx, id, runs(text));
  ctx.doc.setIn(
    ["jobs", id, "steps", i, "run"],
    String(stepsOf(ctx, id)[i].run).replace(from, to),
  );
};
const moveStepOf = (ctx, id, from, to) => {
  const list = ctx.doc.getIn(["jobs", id, "steps"], true).items;
  const [step] = list.splice(from, 1);
  list.splice(to, 0, step);
};

/** The index of the first step that satisfies `pred`. */
const at = (ctx, pred) => {
  const index = steps(ctx).findIndex(pred);
  assert.notEqual(index, -1, "no step matches");
  return index;
};
const checkoutStep = (ctx) =>
  at(ctx, (step) => step.uses?.startsWith("actions/checkout@"));
const runs = (text) => (step) => String(step.run ?? "").includes(text);
const publishStep = (ctx) => at(ctx, runs("gh release create"));

/** A GitHub Actions expression, `${{ … }}`, as the workflow spells it. */
const gh = (expression) => `\${{ ${expression} }}`;
/** An expression with its whitespace collapsed, as YAML folding leaves it. */
const normalise = (text) =>
  String(text ?? "")
    .replace(/\s+/g, " ")
    .replace(/\( /g, "(")
    .replace(/ \)/g, ")")
    .trim();
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
const GUARDED = "steps.guard.outputs.skip != 'true'";
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
const TIMEOUTS = {
  alpha: 20,
  verify: 45,
  build: 45,
  publish: 20,
  "commit-back": 15,
};

const PUBLISH_IF = [
  "(github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v') && !contains(github.ref_name, '-alpha.'))",
  "(github.event_name == 'workflow_dispatch' && inputs.tag != '')",
].join(" || ");
const NOT_DRY_RUN = "needs.verify.outputs.dry_run != 'true'";
const PUBLISH_CONCURRENCY = {
  group: `release-publish-${gh("github.ref")}`,
  "cancel-in-progress": false,
};
const WRITE = { contents: "write" };

/** Sets the alpha job's `if` to the expected one with `clause` removed. */
const ifWithout = (clause) => (ctx) =>
  ctx.doc.setIn(
    [...ALPHA, "if"],
    ALPHA_IF.replace(` && ${clause}`, "").replace(`${clause} && `, ""),
  );

const checks = [
  {
    name: "triggers: workflow_run of ci on main (completed), push of v* tags, and a dispatch with the single input tag",
    check: (ctx) => {
      const on = workflow(ctx).on;
      assert.deepEqual(Object.keys(on).sort(), [
        "push",
        "workflow_dispatch",
        "workflow_run",
      ]);
      assert.deepEqual(on.workflow_run, {
        workflows: ["ci"],
        types: ["completed"],
        branches: ["main"],
      });
      assert.deepEqual(on.push, { tags: ["v*"] });
      const inputs = on.workflow_dispatch?.inputs ?? {};
      assert.deepEqual(Object.keys(inputs), ["tag"]);
      assert.equal(inputs.tag.required, false);
      assert.equal(inputs.tag.default, "");
    },
    mutants: {
      "drops branches": (ctx) =>
        ctx.doc.deleteIn(["on", "workflow_run", "branches"]),
      "adds pull_request": (ctx) => ctx.doc.setIn(["on", "pull_request"], null),
      "follows ci and changes": (ctx) =>
        ctx.doc.setIn(["on", "workflow_run", "workflows"], ["ci", "changes"]),
      "renames the dispatch input": (ctx) => {
        const input = ctx.doc.getIn([
          "on",
          "workflow_dispatch",
          "inputs",
          "tag",
        ]);
        ctx.doc.deleteIn(["on", "workflow_dispatch", "inputs", "tag"]);
        ctx.doc.setIn(["on", "workflow_dispatch", "inputs", "ref"], input);
      },
      "defaults the input to a tag": (ctx) =>
        ctx.doc.setIn(
          ["on", "workflow_dispatch", "inputs", "tag", "default"],
          "v0.1.0",
        ),
    },
  },
  {
    name: "grants contents: read on the workflow and sets permissions per job",
    check: (ctx) => {
      const wf = workflow(ctx);
      assert.deepEqual(wf.permissions, { contents: "read" });
      assert.deepEqual(Object.keys(wf.jobs).sort(), [...JOBS].sort());
      assert.deepEqual(wf.jobs.alpha.permissions, WRITE);
      // verify and build inherit contents: read from the workflow.
      assert.equal(wf.jobs.verify.permissions, undefined);
      assert.equal(wf.jobs.build.permissions, undefined);
      // Exactly contents: write; id-token and attestations come with the
      // registry publishing, which changes this check.
      assert.deepEqual(wf.jobs.publish.permissions, WRITE);
      assert.deepEqual(wf.jobs["commit-back"].permissions, WRITE);
    },
    mutants: {
      "grants write at the top": (ctx) =>
        ctx.doc.setIn(["permissions", "contents"], "write"),
      "drops the alpha permissions": (ctx) =>
        ctx.doc.deleteIn([...ALPHA, "permissions"]),
      "grants the alpha id-token: write": (ctx) =>
        ctx.doc.setIn([...ALPHA, "permissions", "id-token"], "write"),
      "grants verify contents: write": (ctx) =>
        ctx.doc.setIn(["jobs", "verify", "permissions"], WRITE),
      "grants build contents: write": (ctx) =>
        ctx.doc.setIn(["jobs", "build", "permissions"], WRITE),
      "grants publish id-token: write": (ctx) =>
        ctx.doc.setIn(["jobs", "publish", "permissions", "id-token"], "write"),
      "drops the commit-back permissions": (ctx) =>
        ctx.doc.deleteIn(["jobs", "commit-back", "permissions"]),
      "drops the publish permissions": (ctx) =>
        ctx.doc.deleteIn(["jobs", "publish", "permissions"]),
      "adds a job": (ctx) =>
        ctx.doc.setIn(["jobs", "extra"], { "runs-on": "ubuntu-latest" }),
    },
  },
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
            "refs/heads/main",
            "refs/heads/dev",
          ),
        ),
      "reads anonymously": (ctx) => ctx.doc.deleteIn([...STEPS, 0, "env"]),
    },
  },
  {
    name: "runs every step of every job in bash with pipefail",
    check: (ctx) => {
      for (const id of JOBS)
        assert.equal(job(ctx, id).defaults?.run?.shell, "bash", id);
    },
    mutants: {
      "drops the defaults": (ctx) => ctx.doc.deleteIn([...ALPHA, "defaults"]),
      "runs sh": (ctx) =>
        ctx.doc.setIn([...ALPHA, "defaults", "run", "shell"], "sh"),
      "drops the defaults of verify": (ctx) =>
        ctx.doc.deleteIn(["jobs", "verify", "defaults"]),
      "drops the defaults of build": (ctx) =>
        ctx.doc.deleteIn(["jobs", "build", "defaults"]),
      "drops the defaults of publish": (ctx) =>
        ctx.doc.deleteIn(["jobs", "publish", "defaults"]),
      "drops the defaults of commit-back": (ctx) =>
        ctx.doc.deleteIn(["jobs", "commit-back", "defaults"]),
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
    name: "pins every action by full commit SHA with its version as a comment",
    check: (ctx) => {
      const uses = JOBS.flatMap((id) =>
        ctx.doc
          .getIn(["jobs", id, "steps"], true)
          .items.map((step) => step.get("uses", true))
          .filter(Boolean),
      );
      assert.ok(uses.length > 0, "the workflow uses no action");
      for (const node of uses) {
        assert.match(node.value, /^[\w.-]+\/[\w.-]+@[0-9a-f]{40}$/);
        assert.match(node.comment ?? "", /^ v\d+\.\d+\.\d+$/, node.value);
      }
    },
    mutants: {
      "pins checkout by tag": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, checkoutStep(ctx), "uses"],
          "actions/checkout@v7",
        ),
      "drops a version comment": (ctx) => {
        ctx.doc.getIn([...STEPS, checkoutStep(ctx), "uses"], true).comment =
          undefined;
      },
      "pins upload-artifact by tag": (ctx) =>
        ctx.doc.setIn(
          [
            "jobs",
            "build",
            "steps",
            stepAt(ctx, "build", usesAction("actions/upload-artifact")),
            "uses",
          ],
          "actions/upload-artifact@v7",
        ),
      "pins the commit-back checkout by tag": (ctx) =>
        ctx.doc.setIn(
          [
            "jobs",
            "commit-back",
            "steps",
            stepAt(ctx, "commit-back", usesAction("actions/checkout")),
            "uses",
          ],
          "actions/checkout@v7",
        ),
    },
  },
  {
    name: "bounds every job in time",
    check: (ctx) => {
      for (const id of JOBS)
        assert.equal(job(ctx, id)["timeout-minutes"], TIMEOUTS[id], id);
    },
    mutants: {
      "deletes the timeout": (ctx) =>
        ctx.doc.deleteIn([...ALPHA, "timeout-minutes"]),
      "sets the timeout to 0": (ctx) =>
        ctx.doc.setIn([...ALPHA, "timeout-minutes"], 0),
      "deletes the timeout of verify": (ctx) =>
        ctx.doc.deleteIn(["jobs", "verify", "timeout-minutes"]),
      "deletes the timeout of build": (ctx) =>
        ctx.doc.deleteIn(["jobs", "build", "timeout-minutes"]),
      "deletes the timeout of publish": (ctx) =>
        ctx.doc.deleteIn(["jobs", "publish", "timeout-minutes"]),
      "deletes the timeout of commit-back": (ctx) =>
        ctx.doc.deleteIn(["jobs", "commit-back", "timeout-minutes"]),
    },
  },
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
      assert.equal(verify[v].env?.TAG, gh("inputs.tag || github.ref_name"));
      for (const [output, step] of [
        ["stage", "stage"],
        ["version", "version"],
        ["last_beta", "last_beta"],
      ])
        assert.equal(
          job(ctx, "verify").outputs?.[output],
          gh(`steps.verify.outputs.${step}`),
        );
      assert.equal(
        job(ctx, "verify").outputs?.tag,
        gh("inputs.tag || github.ref_name"),
      );
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
        /node scripts\/release\/commit-back\.mjs --tag "\$TAG" --stage "\$STAGE" --version "\$VERSION" --date "\$\(date -u \+%F\)"/,
      );
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
          ' --date "$(date -u +%F)"',
          "",
        ),
    },
  },
  {
    name: "publish and commit-back keep their write token out of the checkout; only the commit-back step gets it",
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
      assert.equal(step.env?.GH_TOKEN, gh("github.token"));
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
    name: "publish concurrency is per ref and never cancels",
    check: (ctx) =>
      assert.deepEqual(job(ctx, "verify").concurrency, PUBLISH_CONCURRENCY),
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
    },
  },
];

runWorkflowChecks({ files: load(), clone, checks });

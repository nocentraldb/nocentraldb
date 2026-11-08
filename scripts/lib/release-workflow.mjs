// What the contract tests of .github/workflows/release.yml share: the parsed
// workflow, the helpers that find and mutate its jobs and steps, and the
// values more than one of them checks. The checks themselves live in
// scripts/release-workflow.test.mjs (the rules for every job),
// scripts/release-workflow-alpha.test.mjs (the alpha job) and
// scripts/release-workflow-publish.test.mjs (verify, build, publish and
// commit-back).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseDocument } from "yaml";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");

export const load = () => {
  const doc = parseDocument(
    readFileSync(join(root, ".github/workflows/release.yml"), "utf8"),
  );
  assert.deepEqual(doc.errors, [], "release.yml does not parse");
  return { doc };
};
export const clone = (ctx) => ({ doc: ctx.doc.clone() });

export const workflow = (ctx) => ctx.doc.toJS();
export const ALPHA = ["jobs", "alpha"];
export const STEPS = [...ALPHA, "steps"];
export const alpha = (ctx) => workflow(ctx).jobs?.alpha ?? {};
export const steps = (ctx) => alpha(ctx).steps ?? [];
export const JOBS = ["alpha", "verify", "build", "publish", "commit-back"];
export const job = (ctx, id) => workflow(ctx).jobs?.[id] ?? {};
export const stepsOf = (ctx, id) => job(ctx, id).steps ?? [];
/** The index of the first step of job `id` that satisfies `pred`. */
export const stepAt = (ctx, id, pred) => {
  const index = stepsOf(ctx, id).findIndex(pred);
  assert.notEqual(index, -1, `no step of ${id} matches`);
  return index;
};
export const usesAction = (name) => (step) =>
  String(step.uses ?? "").startsWith(`${name}@`);
export const runs = (text) => (step) => String(step.run ?? "").includes(text);
/** Replaces `from` by `to` in the run of the first step of `id` that runs `text`. */
export const editRun = (ctx, id, text, from, to) => {
  const i = stepAt(ctx, id, runs(text));
  ctx.doc.setIn(
    ["jobs", id, "steps", i, "run"],
    String(stepsOf(ctx, id)[i].run).replace(from, to),
  );
};
export const moveStepOf = (ctx, id, from, to) => {
  const list = ctx.doc.getIn(["jobs", id, "steps"], true).items;
  const [step] = list.splice(from, 1);
  list.splice(to, 0, step);
};

/** The index of the first step of the alpha job that satisfies `pred`. */
export const at = (ctx, pred) => {
  const index = steps(ctx).findIndex(pred);
  assert.notEqual(index, -1, "no step matches");
  return index;
};
export const checkoutStep = (ctx) =>
  at(ctx, (step) => step.uses?.startsWith("actions/checkout@"));
export const publishStep = (ctx) => at(ctx, runs("gh release create"));

/** A GitHub Actions expression, `${{ … }}`, as the workflow spells it. */
export const gh = (expression) => `\${{ ${expression} }}`;
/** An expression with its whitespace collapsed, as YAML folding leaves it. */
export const normalise = (text) =>
  String(text ?? "")
    .replace(/\s+/g, " ")
    .replace(/\( /g, "(")
    .replace(/ \)/g, ")")
    .trim();

export const GUARDED = "steps.guard.outputs.skip != 'true'";
export const WRITE = { contents: "write" };

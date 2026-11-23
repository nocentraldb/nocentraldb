// Asserts where the memory budget guard runs (NOC-479; Carsten, 2026-11-23,
// NOC-451, question 1 A): only for releases. The `verify` job of
// .github/workflows/release.yml runs it after the full gate, for beta and
// stable tags and in the publish dry run, and skips it for an `-alpha.`
// tag: natively with cargo, and in Chromium in Playwright's image, at the
// version package.json pins and by digest, on the SDK built on the runner.
// No other job and no other workflow runs it, and neither `pnpm gates` nor
// the browser tests of pull requests pick it up: the Rust test is
// #[ignore]d, and the browser test lies outside their globs.
//
// Each check also rejects its mutants (scripts/lib/workflow-checks.mjs).
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, matchesGlob } from "node:path";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";
import {
  imageChecks,
  load as loadImage,
} from "./lib/playwright-image-checks.mjs";
import {
  clone,
  gh,
  JOBS,
  load,
  moveStepOf,
  runs,
  stepAt,
  stepsOf,
} from "./lib/release-workflow.mjs";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (file) => readFileSync(join(root, file), "utf8");

const NATIVE =
  "cargo test --release --locked -p nocentraldb --test memory_budget -- --ignored";
const BROWSER = "node --test scripts/memory-budget/browser.test.mjs";
const BROWSER_FILE = "scripts/memory-budget/browser.test.mjs";
const RUST_FILE = "crates/nocentraldb/tests/memory_budget.rs";
const SDK_BUILD = "node scripts/sdk-browser/build.mjs";
const WASM_RUNNER = "moon run repo:wasm-runner";
const NOT_ALPHA = gh("!contains(env.TAG, '-alpha.')");

/** The step of verify that runs `text`, and its index. */
const verifyStep = (ctx, text) => {
  const i = stepAt(ctx, "verify", runs(text));
  return { i, step: stepsOf(ctx, "verify")[i] };
};
const setStep = (ctx, i, key, value) =>
  ctx.doc.setIn(["jobs", "verify", "steps", i, key], value);

const checks = [
  {
    name: "verify runs the native guard after the full gate, and not for an alpha tag",
    check: (ctx) => {
      const gates = stepAt(ctx, "verify", (s) => s.run === "pnpm gates");
      const { i, step } = verifyStep(ctx, NATIVE);
      assert.equal(step.run.trim(), NATIVE);
      assert.ok(gates < i, "the guard runs before the gate");
      assert.equal(step.if, NOT_ALPHA);
    },
    mutants: {
      "drops the alpha exclusion": (ctx) =>
        ctx.doc.deleteIn([
          "jobs",
          "verify",
          "steps",
          verifyStep(ctx, NATIVE).i,
          "if",
        ]),
      "runs it only for alphas": (ctx) =>
        setStep(
          ctx,
          verifyStep(ctx, NATIVE).i,
          "if",
          gh("contains(env.TAG, '-alpha.')"),
        ),
      "runs it without --ignored": (ctx) =>
        setStep(
          ctx,
          verifyStep(ctx, NATIVE).i,
          "run",
          `${NATIVE} --exact`.replace(" -- --ignored", ""),
        ),
      "runs it in a debug build": (ctx) =>
        setStep(
          ctx,
          verifyStep(ctx, NATIVE).i,
          "run",
          NATIVE.replace(" --release", " ").replace("  ", " "),
        ),
      "runs it before the gate": (ctx) =>
        moveStepOf(
          ctx,
          "verify",
          verifyStep(ctx, NATIVE).i,
          stepAt(ctx, "verify", (s) => s.run === "pnpm gates"),
        ),
    },
  },
  {
    name: "verify builds the SDK after wasm-bindgen and runs the browser guard in Playwright's image, not for an alpha tag",
    check: (ctx) => {
      const gates = stepAt(ctx, "verify", (s) => s.run === "pnpm gates");
      const build = verifyStep(ctx, SDK_BUILD);
      const pull = verifyStep(ctx, "docker pull");
      const guard = verifyStep(ctx, BROWSER);
      const runner = build.step.run.indexOf(WASM_RUNNER);
      assert.ok(gates < build.i, "the SDK is built before the gate");
      assert.ok(
        runner !== -1 && runner < build.step.run.indexOf(SDK_BUILD),
        "the SDK is built before wasm-bindgen is installed",
      );
      assert.ok(build.i < guard.i, "the guard runs before the SDK is built");
      assert.ok(pull.i < guard.i, "the guard runs before the image is pulled");
      for (const { step } of [build, pull, guard])
        assert.equal(step.if, NOT_ALPHA, step.name);
      assert.match(guard.step.run, /^docker run --rm --ipc=host\b/);
      assert.match(guard.step.run, /-e SDK_BROWSER_PREBUILT=1\b/);
      assert.match(guard.step.run, / "\$PLAYWRIGHT_IMAGE" /);
      assert.ok(guard.step.run.trim().endsWith(BROWSER));
    },
    mutants: {
      "drops the alpha exclusion of the guard": (ctx) =>
        ctx.doc.deleteIn([
          "jobs",
          "verify",
          "steps",
          verifyStep(ctx, BROWSER).i,
          "if",
        ]),
      "drops the alpha exclusion of the build": (ctx) =>
        ctx.doc.deleteIn([
          "jobs",
          "verify",
          "steps",
          verifyStep(ctx, SDK_BUILD).i,
          "if",
        ]),
      "runs the guard on the runner, not in the image": (ctx) =>
        setStep(ctx, verifyStep(ctx, BROWSER).i, "run", BROWSER),
      "rebuilds in the image": (ctx) => {
        const { i, step } = verifyStep(ctx, BROWSER);
        setStep(
          ctx,
          i,
          "run",
          step.run.replace("-e SDK_BROWSER_PREBUILT=1 ", ""),
        );
      },
      "runs the guard before the build": (ctx) =>
        moveStepOf(
          ctx,
          "verify",
          verifyStep(ctx, BROWSER).i,
          verifyStep(ctx, SDK_BUILD).i,
        ),
      "drops wasm-bindgen": (ctx) =>
        setStep(ctx, verifyStep(ctx, SDK_BUILD).i, "run", SDK_BUILD),
    },
  },
  {
    name: "no other job of release.yml runs the guards",
    check: (ctx) => {
      for (const id of JOBS.filter((j) => j !== "verify"))
        for (const step of stepsOf(ctx, id))
          assert.doesNotMatch(
            String(step.run ?? ""),
            /memory_budget|memory-budget/,
            id,
          );
    },
    mutants: {
      "build runs the native guard": (ctx) =>
        ctx.doc.addIn(["jobs", "build", "steps"], { run: NATIVE }),
      "alpha runs the browser guard": (ctx) =>
        ctx.doc.addIn(["jobs", "alpha", "steps"], { run: BROWSER }),
    },
  },
];

runWorkflowChecks({ files: load(), clone, checks });

// The Playwright image release.yml names, as every browser workflow.
runWorkflowChecks({
  files: loadImage(".github/workflows/release.yml"),
  clone: (ctx) => structuredClone(ctx),
  checks: imageChecks(),
});

/** What the guard's placement depends on outside release.yml. */
const outside = () => {
  const workflows = Object.fromEntries(
    readdirSync(join(root, ".github/workflows"))
      .filter((f) => f.endsWith(".yml"))
      .map((f) => [f, read(`.github/workflows/${f}`)]),
  );
  return {
    workflows,
    rust: read(RUST_FILE),
    globs: [
      // repo:test (moon.yml), run by `pnpm gates`.
      ...parse(read("moon.yml"))
        .tasks.test.command.match(/'[^']+'/g)
        .map((g) => g.slice(1, -1)),
      // The TypeScript SDK's browser tests (browser-view.yml).
      ...[
        ...read(".github/workflows/browser-view.yml").matchAll(
          /node --test '([^']+)'/g,
        ),
      ].map((m) => m[1]),
    ],
  };
};

runWorkflowChecks({
  files: outside(),
  clone: (ctx) => structuredClone(ctx),
  checks: [
    {
      name: "no workflow but release.yml runs the guards",
      check: (ctx) => {
        for (const [file, text] of Object.entries(ctx.workflows))
          if (file !== "release.yml")
            assert.doesNotMatch(text, /memory_budget|memory-budget/, file);
      },
      mutants: {
        "ci runs the native guard": (ctx) => {
          ctx.workflows["ci.yml"] += `\n      - run: ${NATIVE}\n`;
        },
        "browser-view runs the browser guard": (ctx) => {
          ctx.workflows["browser-view.yml"] += `\n      - run: ${BROWSER}\n`;
        },
      },
    },
    {
      name: "neither the gate nor the browser tests of pull requests pick the guards up",
      check: (ctx) => {
        assert.ok(ctx.globs.length >= 3, "the globs were not found");
        for (const glob of ctx.globs)
          assert.ok(!matchesGlob(BROWSER_FILE, glob), `${glob} runs it`);
        assert.match(
          ctx.rust,
          /#\[test\]\n#\[ignore = "release-only[^"]*"\]\nfn /,
          `${RUST_FILE}: cargo test would run it`,
        );
      },
      mutants: {
        "the gate runs every test under scripts/": (ctx) => {
          ctx.globs.push("scripts/**/*.test.mjs");
        },
        "the Rust guard is not ignored": (ctx) => {
          ctx.rust = ctx.rust.replace(/#\[ignore = "[^"]*"\]\n/, "");
        },
      },
    },
  ],
});

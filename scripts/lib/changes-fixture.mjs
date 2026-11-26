// The fixture the run tests of .github/workflows/changes.yml share
// (scripts/changes-workflow-merge-ref.test.mjs for a pull request's merge
// ref, scripts/changes-workflow-merge-queue.test.mjs for the merge queue's
// commit). It holds the workflow's steps, a small repository with a
// TypeScript package whose API report the workflow diffs, and `runStep`,
// which runs one step's `run` script as GitHub runs it (bash -eo pipefail)
// with the event's values in its env.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";
import { GIT_ENV, makeRepo } from "../release/lib/test-repo.mjs";

export const root = join(dirname(fileURLToPath(import.meta.url)), "../..");

const steps = parse(
  readFileSync(join(root, ".github/workflows/changes.yml"), "utf8"),
).jobs.changes.steps;

/** The step of changes.yml that satisfies `pred`. */
export const step = (pred, what) => {
  const found = steps.find(pred);
  assert.ok(found, `changes.yml has no ${what} step`);
  return found;
};

export const FEATURE =
  "---\ntype: feature\ntickets: [mt08.1qaz]\ncomponents: [core]\n---\n## Problem\na\n## Use\nb\n";
export const BREAKING =
  "---\ntype: breaking\ntickets: [mt08.1qaz]\ncomponents: [core]\n---\n## Problem\na\n## Use\nb\n## Migration\nc\n";

const A = "export const name = (): string => 'sdk';\n";
const B =
  "export const other = (): number => 1;\nexport const more = (): number => 2;\n";

// Main's breaking change: it removes `other`, with its own breaking fragment.
export const MAIN_BREAKS = {
  "packages/sdk/src/b.ts": "export const more = (): number => 2;\n",
  "changes/main-drops-other.md": BREAKING,
};

const writeReport = (repo) => {
  const result = spawnSync(
    process.execPath,
    [join(root, "scripts/release/api-report.mjs"), "--write"],
    { cwd: repo.dir, env: GIT_ENV, encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
};

/**
 * A repository on `main` with a TypeScript package `sdk` and its API report,
 * and `scripts/` linked in, as the workflow runs `node scripts/...` from the
 * repository root. `commitWith(message, files)` writes the files,
 * regenerates the report, as the gate requires, and commits; `report()`
 * regenerates it alone, for a merge the test resolves itself.
 */
export function packageRepo(t) {
  const repo = makeRepo();
  t.after(() => repo.dispose());
  repo.write(
    "tsconfig.base.json",
    readFileSync(join(root, "tsconfig.base.json"), "utf8"),
  );
  repo.write(
    "packages/sdk/tsconfig.json",
    `${JSON.stringify({ extends: "../../tsconfig.base.json", include: ["src"] })}\n`,
  );
  repo.write(
    "packages/sdk/src/index.ts",
    'export * from "./a.ts";\nexport * from "./b.ts";\n',
  );
  repo.write("packages/sdk/src/a.ts", A);
  repo.write("packages/sdk/src/b.ts", B);
  symlinkSync(join(root, "scripts"), join(repo.dir, "scripts"));
  writeFileSync(join(repo.dir, ".git/info/exclude"), "/scripts\n");
  const commitWith = (message, files) => {
    for (const [file, text] of Object.entries(files)) repo.write(file, text);
    writeReport(repo);
    return repo.commit(message);
  };
  return { repo, commitWith, report: () => writeReport(repo) };
}

/**
 * Runs one step's script as GitHub does; returns its status and outputs.
 * `values` maps each expression of the step's env to its value; `env` adds
 * the runner's own variables, such as PATH or GITHUB_REPOSITORY.
 */
export const runStep = (t, repo, theStep, values, env = {}) => {
  const temp = mkdtempSync(join(tmpdir(), "changes-runner-"));
  t.after(() => rmSync(temp, { recursive: true, force: true }));
  const output = join(temp, "output");
  writeFileSync(output, "");
  const all = { ...GIT_ENV, RUNNER_TEMP: temp, GITHUB_OUTPUT: output, ...env };
  for (const [name, expression] of Object.entries(theStep.env ?? {})) {
    assert.ok(expression in values, `unknown expression ${expression}`);
    all[name] = values[expression];
  }
  const result = spawnSync(
    "bash",
    ["--noprofile", "--norc", "-eo", "pipefail", "-c", theStep.run],
    { cwd: repo.dir, env: all, encoding: "utf8" },
  );
  const outputs = Object.fromEntries(
    readFileSync(output, "utf8")
      .split("\n")
      .filter(Boolean)
      .map((line) => line.split("=", 2)),
  );
  return { ...result, outputs, temp };
};

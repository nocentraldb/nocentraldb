import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { removedLines } from "./api-report.mjs";
import { GIT_ENV, makeRepo } from "./lib/test-repo.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const script = join(here, "api-report.mjs");
const root = join(here, "..", "..");

const REPORT = "packages/sdk/etc/api-report.d.ts";
const NAME = "export declare function name(): string;";

const INDEX = `/** Returns the package name. */
export function name(): string {
  return "@nocentraldb/sdk";
}
`;

/**
 * A fixture copied from packages/sdk: its own tsconfig extending a copy of
 * the base config, one exported function, and a test file whose declarations
 * must stay out of the report.
 */
const FIXTURE = {
  "tsconfig.base.json": readFileSync(join(root, "tsconfig.base.json"), "utf8"),
  "packages/sdk/tsconfig.json": `${JSON.stringify(
    { extends: "../../tsconfig.base.json", include: ["src", "test"] },
    null,
    2,
  )}\n`,
  "packages/sdk/src/index.ts": INDEX,
  "packages/sdk/test/smoke.test.ts": "export const probe = 1;\n",
};

const fixture = (t) => {
  const repo = makeRepo();
  t.after(() => repo.dispose());
  for (const [file, text] of Object.entries(FIXTURE)) repo.write(file, text);
  return repo;
};

const run = (repo, ...args) =>
  spawnSync(process.execPath, [script, ...args], {
    cwd: repo.dir,
    env: GIT_ENV,
    encoding: "utf8",
  });

const report = (repo) => readFileSync(join(repo.dir, REPORT), "utf8");

/** Writes the report and commits the fixture; returns the commit. */
const committed = (t, extra = {}) => {
  const repo = fixture(t);
  for (const [file, text] of Object.entries(extra)) repo.write(file, text);
  assert.equal(run(repo, "--write").status, 0);
  const base = repo.commit("base");
  return { repo, base };
};

test("--write produces the declarations of src/ under a file header, without test/", (t) => {
  const repo = fixture(t);
  const result = run(repo, "--write");
  assert.equal(result.status, 0, result.stderr);
  const text = report(repo);
  assert.match(text, /^\/\/ file: index\.d\.ts\n/m);
  assert.ok(text.includes(`// file: index.d.ts\n${NAME}\n`), text);
  assert.ok(!text.includes("probe"), "the test directory leaked in");
  assert.ok(!text.includes("\r"), "the report is not LF-normalised");
});

test("--write orders the files by path, each with its header", (t) => {
  const repo = fixture(t);
  repo.write("packages/sdk/src/b/z.ts", "export const z = 1;\n");
  repo.write("packages/sdk/src/a.ts", "export const a = 1;\n");
  repo.write(
    "packages/sdk/src/index.ts",
    `${INDEX}export { a } from "./a.ts";\nexport * from "./b/z.ts";\n`,
  );
  assert.equal(run(repo, "--write").status, 0);
  const headers = report(repo)
    .split("\n")
    .filter((line) => line.startsWith("// file: "));
  assert.deepEqual(headers, [
    "// file: a.d.ts",
    "// file: b/z.d.ts",
    "// file: index.d.ts",
  ]);
});

test("a file src/index.ts does not reach stays out of the report (NOC-176)", (t) => {
  const repo = fixture(t);
  repo.write("packages/sdk/src/internal.ts", "export const internal = 1;\n");
  assert.equal(run(repo, "--write").status, 0);
  const text = report(repo);
  assert.ok(!text.includes("internal"), text);
  assert.deepEqual(
    text.split("\n").filter((line) => line.startsWith("// file: ")),
    ["// file: index.d.ts"],
  );
});

test("the report follows the declarations' imports and re-exports, transitively", (t) => {
  const repo = fixture(t);
  repo.write(
    "packages/sdk/src/index.ts",
    `${INDEX}export type { Db } from "./db.ts";\nexport * as nc from "./nc.ts";\n`,
  );
  repo.write(
    "packages/sdk/src/db.ts",
    'import type { Id } from "./id.ts";\nimport { run } from "./impl.ts";\nexport interface Db { id: Id }\nexport const open = (): Db => run();\n',
  );
  repo.write("packages/sdk/src/id.ts", "export type Id = string;\n");
  repo.write("packages/sdk/src/nc.ts", 'export { open } from "./db.ts";\n');
  // Imported only for a value: its declarations import nothing from it.
  repo.write(
    "packages/sdk/src/impl.ts",
    'import type { Db } from "./db.ts";\nexport const run = (): Db => ({ id: "x" });\nexport const hidden = 2;\n',
  );
  assert.equal(run(repo, "--write").status, 0);
  const headers = report(repo)
    .split("\n")
    .filter((line) => line.startsWith("// file: "));
  assert.deepEqual(headers, [
    "// file: db.d.ts",
    "// file: id.d.ts",
    "// file: index.d.ts",
    "// file: nc.d.ts",
  ]);
  assert.ok(!report(repo).includes("hidden"));
});

test("a project without src/index.ts fails, naming the entry", (t) => {
  const repo = fixture(t);
  repo.remove("packages/sdk/src/index.ts");
  repo.write("packages/sdk/src/other.ts", "export const other = 1;\n");
  const result = run(repo, "--write");
  assert.equal(result.status, 1);
  assert.match(result.stderr, /src\/index\.ts/);
});

test("check mode passes on a fresh report", (t) => {
  const repo = fixture(t);
  assert.equal(run(repo, "--write").status, 0);
  const result = run(repo);
  assert.equal(result.status, 0, result.stderr);
});

test("check mode fails when src/index.ts gains an export without --write", (t) => {
  const repo = fixture(t);
  assert.equal(run(repo, "--write").status, 0);
  repo.write(
    "packages/sdk/src/index.ts",
    `${INDEX}export function other(): number {\n  return 1;\n}\n`,
  );
  const result = run(repo);
  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /api-report: packages\/sdk\/etc\/api-report\.d\.ts is out of date; run node scripts\/release\/api-report\.mjs --write/,
  );
});

test("check mode fails when the report is missing", (t) => {
  const repo = fixture(t);
  const result = run(repo);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /api-report\.d\.ts is out of date/);
});

test("a type error fails the run with tsc's output", (t) => {
  const repo = fixture(t);
  repo.write("packages/sdk/src/index.ts", "export const x: number = 'no';\n");
  const result = run(repo, "--write");
  assert.equal(result.status, 1);
  assert.match(result.stderr, /api-report: tsc failed/);
  assert.match(result.stdout + result.stderr, /TS2322/);
});

test("--base: removing the export exits 3 and names the removed line", (t) => {
  const { repo, base } = committed(t);
  repo.write("packages/sdk/src/index.ts", "export const kept = 1;\n");
  assert.equal(run(repo, "--write").status, 0);
  repo.commit("head");
  const result = run(repo, "--base", base);
  assert.equal(result.status, 3, result.stderr);
  assert.match(
    result.stdout,
    new RegExp(`api-report: breaking: - ${esc(NAME)}`),
  );
});

test("--base: adding an export exits 0", (t) => {
  const { repo, base } = committed(t);
  repo.write(
    "packages/sdk/src/index.ts",
    `${INDEX}export function other(): number {\n  return 1;\n}\n`,
  );
  assert.equal(run(repo, "--write").status, 0);
  repo.commit("head");
  const result = run(repo, "--base", base);
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.doesNotMatch(result.stdout, /breaking/);
});

test("--base: changing a signature exits 3", (t) => {
  const { repo, base } = committed(t);
  repo.write(
    "packages/sdk/src/index.ts",
    "export function name(prefix: string): string {\n  return prefix;\n}\n",
  );
  assert.equal(run(repo, "--write").status, 0);
  repo.commit("head");
  const result = run(repo, "--base", base);
  assert.equal(result.status, 3);
  assert.match(result.stdout, new RegExp(`breaking: - ${esc(NAME)}`));
});

test("--base: a report absent at the base counts as all additions", (t) => {
  const repo = fixture(t);
  const base = repo.commit("base without a report");
  assert.equal(run(repo, "--write").status, 0);
  repo.commit("head");
  const result = run(repo, "--base", base);
  assert.equal(result.status, 0, result.stdout + result.stderr);
});

test("--base: a stale report is reported as out of date (1) before any diff", (t) => {
  const { repo, base } = committed(t);
  repo.write("packages/sdk/src/index.ts", "export const kept = 1;\n");
  const result = run(repo, "--base", base);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /is out of date/);
});

test("--base: a doc comment change is not breaking", (t) => {
  const { repo, base } = committed(t);
  repo.write(
    "packages/sdk/src/index.ts",
    INDEX.replace("Returns the package name.", "The package's name."),
  );
  assert.equal(run(repo, "--write").status, 0);
  repo.commit("head");
  const result = run(repo, "--base", base);
  assert.equal(result.status, 0, result.stdout + result.stderr);
});

test("--base: an unknown revision fails (1), naming it", (t) => {
  const { repo } = committed(t);
  const result = run(repo, "--base", "no-such-rev");
  assert.equal(result.status, 1);
  assert.match(result.stderr, /api-report: .*no-such-rev/);
});

test("--project points at another project directory", (t) => {
  const repo = fixture(t);
  repo.write(
    "libs/other/tsconfig.json",
    '{ "extends": "../../tsconfig.base.json", "include": ["src"] }\n',
  );
  repo.write("libs/other/src/index.ts", "export const other = 1;\n");
  const result = run(repo, "--write", "--project", "libs/other");
  assert.equal(result.status, 0, result.stderr);
  const text = readFileSync(
    join(repo.dir, "libs/other/etc/api-report.d.ts"),
    "utf8",
  );
  assert.match(text, /export declare const other = 1;/);
});

test("--write with --base and unknown options are usage errors (2)", (t) => {
  const repo = fixture(t);
  assert.equal(run(repo, "--write", "--base", "HEAD").status, 2);
  assert.equal(run(repo, "--bogus").status, 2);
});

test("--help prints the usage", (t) => {
  const repo = fixture(t);
  const result = run(repo, "--help");
  assert.equal(result.status, 0);
  assert.match(result.stdout, /usage: api-report/);
});

const esc = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

test("removedLines: lines of closing brackets, blank lines and export {}; are never reported", () => {
  const before = "interface A {\n  a: string;\n}\n\nexport {};\n]);\n";
  const after = "interface A {\n  a: string;\n";
  assert.deepEqual(removedLines(before, after), []);
});

test("removedLines: counts duplicates, so one of two equal lines removed is reported", () => {
  const line = "    id: string;";
  assert.deepEqual(removedLines(`${line}\n${line}\n`, `${line}\n`), [line]);
  assert.deepEqual(removedLines(`${line}\n`, `${line}\n${line}\n`), []);
});

test("--base: removing a member of a multi-line interface exits 3 and names it", (t) => {
  const shape = (members) =>
    `${INDEX}export interface Options {\n${members}}\n`;
  const { repo, base } = committed(t, {
    "packages/sdk/src/index.ts": shape("  id: string;\n  label?: string;\n"),
  });
  repo.write("packages/sdk/src/index.ts", shape("  id: string;\n"));
  assert.equal(run(repo, "--write").status, 0);
  repo.commit("head");
  const result = run(repo, "--base", base);
  assert.equal(result.status, 3, result.stderr);
  assert.match(result.stdout, /api-report: breaking: - {5}label\?: string;/);
  assert.doesNotMatch(result.stdout, /id: string/);
});

test("a tsc that does not finish in time fails the run with the cause", (t) => {
  const repo = fixture(t);
  const result = spawnSync(process.execPath, [script, "--write"], {
    cwd: repo.dir,
    env: { ...GIT_ENV, API_REPORT_TSC_TIMEOUT_MS: "1" },
    encoding: "utf8",
  });
  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /api-report: tsc did not run to the end .*ETIMEDOUT/,
  );
});

test("removedLines: a line moved to another declaration is not reported (documented blind spot)", () => {
  const before = "interface A {\n  id: string;\n}\ninterface B {\n}\n";
  const after = "interface A {\n}\ninterface B {\n  id: string;\n}\n";
  assert.deepEqual(removedLines(before, after), []);
});

test("--base with a value that starts with a dash is a usage error (2)", (t) => {
  const { repo } = committed(t);
  const result = run(repo, "--base", "--output=/tmp/x");
  assert.equal(result.status, 2);
});

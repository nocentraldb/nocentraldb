// Runs check-pins.mjs against throwaway fixture trees and asserts its exit
// code and message for every rule it enforces.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

const script = join(dirname(fileURLToPath(import.meta.url)), "check-pins.mjs");
const fixtures = [];
after(() => {
  for (const dir of fixtures) rmSync(dir, { recursive: true, force: true });
});

const PROTOTOOLS = 'moon = "2.6.5"\nnode = "24.22.2"\npnpm = "12.17.0"\n';

/** Writes a valid tree, applies the overrides (null deletes a file), runs the check. */
function run(overrides = {}) {
  const dir = mkdtempSync(join(tmpdir(), "check-pins-"));
  fixtures.push(dir);
  const files = {
    ".prototools": PROTOTOOLS,
    ".nvmrc": "24.22.2\n",
    "package.json": {
      packageManager: "pnpm@12.17.0",
      devDependencies: { vitest: "5.0.5" },
    },
    "packages/sdk/package.json": { name: "@x/sdk" },
    ...overrides,
  };
  for (const [file, content] of Object.entries(files)) {
    if (content === null) continue;
    mkdirSync(dirname(join(dir, file)), { recursive: true });
    const text =
      typeof content === "string" ? content : JSON.stringify(content);
    writeFileSync(join(dir, file), text);
  }
  const result = spawnSync(process.execPath, [script], {
    cwd: dir,
    encoding: "utf8",
  });
  return { status: result.status, stderr: result.stderr };
}

const withDep = (spec, field = "dependencies") => ({
  name: "@x/sdk",
  [field]: { dep: spec },
});

test("a fully pinned tree passes", () => {
  assert.deepEqual(run(), { status: 0, stderr: "" });
});

test("a range in the root package.json fails", () => {
  const { status, stderr } = run({
    "package.json": {
      packageManager: "pnpm@12.17.0",
      devDependencies: { vitest: "^5.0.5" },
    },
  });
  assert.equal(status, 1);
  assert.match(
    stderr,
    /package\.json devDependencies\.vitest = "\^5\.0\.5" is not exact/,
  );
});

test("a range in packages/*/package.json fails", () => {
  const { status, stderr } = run({
    "packages/sdk/package.json": withDep("~1.0.0"),
  });
  assert.equal(status, 1);
  assert.match(
    stderr,
    /packages\/sdk\/package\.json dependencies\.dep = "~1\.0\.0" is not exact/,
  );
});

test("a range in apps/*/package.json fails", () => {
  const { status, stderr } = run({
    "apps/site/package.json": withDep(">=1.0.0", "peerDependencies"),
  });
  assert.equal(status, 1);
  assert.match(stderr, /apps\/site\/package\.json peerDependencies\.dep/);
});

for (const spec of [
  "^1.0.0",
  "~1.0.0",
  ">=1.0.0",
  "1.0.0 - 2.0.0",
  "1.0.0 || 2.0.0",
  "*",
  "x",
  "1",
  "1.x",
  "1.2",
  "latest",
  "next",
  "git+https://github.com/a/b.git#main",
  "github:a/b",
  "file:../b",
  "link:../b",
  "npm:other@^1.0.0",
  "https://example.com/b.tgz",
  "",
]) {
  test(`the non-exact spec "${spec}" fails`, () => {
    const { status, stderr } = run({
      "packages/sdk/package.json": withDep(spec, "optionalDependencies"),
    });
    assert.equal(status, 1);
    assert.match(stderr, /optionalDependencies\.dep = .* is not exact/);
  });
}

for (const spec of ["1.2.3", "0.0.0", "1.2.3-beta.1", "1.2.3-rc.0+build.5"]) {
  test(`the exact version "${spec}" passes`, () => {
    const { status } = run({ "packages/sdk/package.json": withDep(spec) });
    assert.equal(status, 0);
  });
}

test("the workspace protocol passes: it names a project in this repo", () => {
  const { status } = run({
    "packages/sdk/package.json": withDep("workspace:*"),
  });
  assert.equal(status, 0);
});

test(".nvmrc differing from the node pin fails", () => {
  const { status, stderr } = run({ ".nvmrc": "24.22.1\n" });
  assert.equal(status, 1);
  assert.match(stderr, /\.nvmrc 24\.22\.1 != \.prototools node 24\.22\.2/);
});

test("packageManager differing from the pnpm pin fails", () => {
  const { status, stderr } = run({
    "package.json": { packageManager: "pnpm@12.16.1" },
  });
  assert.equal(status, 1);
  assert.match(
    stderr,
    /packageManager pnpm@12\.16\.1 != pnpm@12\.17\.0 from \.prototools/,
  );
});

for (const key of ["node", "pnpm"]) {
  test(`a .prototools without a ${key} pin fails with a missing-key message`, () => {
    const prototools = PROTOTOOLS.replace(new RegExp(`^${key} .*\\n`, "m"), "");
    const { status, stderr } = run({ ".prototools": prototools });
    assert.equal(status, 1);
    assert.match(stderr, new RegExp(`\\.prototools has no ${key} pin`));
    assert.doesNotMatch(stderr, /undefined/);
  });
}

test("a missing .nvmrc fails with a message, not a stack trace", () => {
  const { status, stderr } = run({ ".nvmrc": null });
  assert.equal(status, 1);
  assert.match(stderr, /check-pins: cannot read \.nvmrc/);
});

// Runs session-start-deps.mjs in throwaway directories with a fake `pnpm` on
// PATH (NOC-65). A cloud session's node_modules comes from the environment
// snapshot; a dependency added to pnpm-lock.yaml after the snapshot was built
// is missing until someone runs `pnpm install`. The script installs from the
// lockfile when it differs from the one node_modules was installed from.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

const script = join(
  dirname(fileURLToPath(import.meta.url)),
  "session-start-deps.mjs",
);
const fixtures = [];
after(() => {
  for (const dir of fixtures) rmSync(dir, { recursive: true, force: true });
});

/**
 * A checkout with a lockfile and a fake `pnpm` that records each call's
 * arguments in calls.log and exits with `exitCode`.
 */
function checkout({ exitCode = 0 } = {}) {
  const dir = mkdtempSync(join(tmpdir(), "session-start-deps-"));
  fixtures.push(dir);
  const bin = join(dir, "bin");
  mkdirSync(bin);
  const log = join(dir, "calls.log");
  writeFileSync(
    join(bin, "pnpm"),
    `#!/bin/sh\necho "$*" >> "${log}"\necho "pnpm says hello"\nexit ${exitCode}\n`,
  );
  chmodSync(join(bin, "pnpm"), 0o755);
  writeFileSync(join(dir, "pnpm-lock.yaml"), "lockfileVersion: '9.0'\n");
  mkdirSync(join(dir, "node_modules"));
  return { dir, bin, log };
}

function run({ dir, bin }, env = { CLAUDE_CODE_REMOTE: "true" }) {
  const result = spawnSync(process.execPath, [script], {
    cwd: dir,
    encoding: "utf8",
    env: { PATH: `${bin}:${process.env.PATH}`, HOME: process.env.HOME, ...env },
  });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout + result.stderr;
}

const calls = ({ log }) =>
  existsSync(log) ? readFileSync(log, "utf8").trim().split("\n") : [];

test("a lockfile node_modules was not installed from is installed, frozen", () => {
  const fixture = checkout();
  const out = run(fixture);
  assert.deepEqual(calls(fixture), ["install --frozen-lockfile"]);
  assert.match(out, /pnpm install --frozen-lockfile/);
});

test("the same lockfile is not installed twice, silently", () => {
  const fixture = checkout();
  run(fixture);
  assert.equal(run(fixture), "");
  assert.deepEqual(calls(fixture), ["install --frozen-lockfile"]);
});

test("a lockfile changed since the last install is installed again", () => {
  const fixture = checkout();
  run(fixture);
  writeFileSync(
    join(fixture.dir, "pnpm-lock.yaml"),
    "lockfileVersion: '9.0'\n# a new dependency\n",
  );
  run(fixture);
  assert.equal(calls(fixture).length, 2);
});

test("a failed install says so, exits 0 and is tried again next time", () => {
  const fixture = checkout({ exitCode: 1 });
  const out = run(fixture);
  assert.match(out, /failed/);
  assert.match(out, /pnpm says hello/);
  run(fixture);
  assert.equal(calls(fixture).length, 2);
});

test("a missing pnpm says so and exits 0", () => {
  const fixture = checkout();
  const out = run(
    { dir: fixture.dir, bin: fixture.bin },
    { CLAUDE_CODE_REMOTE: "true", PATH: "/nonexistent" },
  );
  assert.match(out, /failed/);
});

test("outside a cloud session the script installs nothing", () => {
  const fixture = checkout();
  assert.equal(run(fixture, {}), "");
  assert.deepEqual(calls(fixture), []);
});

test("a checkout without a lockfile installs nothing", () => {
  const fixture = checkout();
  rmSync(join(fixture.dir, "pnpm-lock.yaml"));
  assert.equal(run(fixture), "");
  assert.deepEqual(calls(fixture), []);
});

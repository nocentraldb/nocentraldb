// Runs check-file-size.mjs in throwaway git repositories and asserts its exit
// code and message: a tracked code or test file past the limit in
// factory/CONFIG.md fails the gate, by name and line count.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

const script = join(
  dirname(fileURLToPath(import.meta.url)),
  "check-file-size.mjs",
);
const fixtures = [];
after(() => {
  for (const dir of fixtures) rmSync(dir, { recursive: true, force: true });
});

const config = (limit) =>
  [
    "# CONFIG.md",
    "",
    "| Setting                   | Value                                   |",
    "| ------------------------- | --------------------------------------- |",
    "| Gate command              | `pnpm gates`                            |",
    `| Largest code or test file | ${limit} lines. A longer file is split. |`,
    "",
  ].join("\n");

/** A file of exactly `count` lines, each ending in a newline. */
const lines = (count) => "x\n".repeat(count);

/**
 * Writes the files into a new git repository, stages them all so they are
 * tracked, and runs the check from its root. CONFIG.md sets a limit of 10
 * unless the files say otherwise; null leaves a file out.
 */
function run(files) {
  const dir = mkdtempSync(join(tmpdir(), "check-file-size-"));
  fixtures.push(dir);
  const tree = { "factory/CONFIG.md": config(10), ...files };
  for (const [file, content] of Object.entries(tree)) {
    if (content === null) continue;
    mkdirSync(dirname(join(dir, file)), { recursive: true });
    writeFileSync(join(dir, file), content);
  }
  const git = (...args) =>
    assert.equal(spawnSync("git", args, { cwd: dir }).status, 0);
  git("init", "--quiet");
  git("add", "--force", ".");
  const result = spawnSync(process.execPath, [script], {
    cwd: dir,
    encoding: "utf8",
  });
  return { status: result.status, stderr: result.stderr };
}

test("code and test files at the limit pass", () => {
  assert.deepEqual(
    run({ "src/at-limit.rs": lines(10), "scripts/a.test.mjs": lines(3) }),
    { status: 0, stderr: "" },
  );
});

test("a file past the limit fails, named with its line count", () => {
  const { status, stderr } = run({
    "src/ok.rs": lines(10),
    "scripts/big.test.mjs": lines(11),
  });
  assert.equal(status, 1);
  assert.match(stderr, /scripts\/big\.test\.mjs has 11 lines/);
  assert.match(stderr, /limit is 10/);
  assert.match(stderr, /split it by concern/i);
  assert.match(stderr, /FACTORY\.md/);
  assert.doesNotMatch(stderr, /ok\.rs/);
});

test("every code extension is checked, and every long file is named", () => {
  const extensions = ["rs", "ts", "tsx", "js", "mjs", "cjs", "sh", "py"];
  const files = Object.fromEntries(
    extensions.map((ext) => [`src/long.${ext}`, lines(12)]),
  );
  const { status, stderr } = run(files);
  assert.equal(status, 1);
  for (const ext of extensions) {
    assert.match(stderr, new RegExp(`src/long\\.${ext} has 12 lines`));
  }
});

test("a last line without a newline counts as a line", () => {
  assert.equal(run({ "a.rs": `${lines(10)}x` }).status, 1);
  assert.equal(run({ "a.rs": lines(10).slice(0, -1) }).status, 0);
});

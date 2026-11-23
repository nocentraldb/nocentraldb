// Runs check-test-changes.mjs in throwaway git repositories (NOC-508): a
// base branch `main` with the files before the pull request, and a branch
// with the pull request's commits on top. It asserts which changes to tests
// the check flags, that a flagged pull request passes only with a
// `Test-change: <reason>` trailer, and that one which only adds tests passes
// untouched.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

const script = join(
  dirname(fileURLToPath(import.meta.url)),
  "check-test-changes.mjs",
);
const fixtures = [];
after(() => {
  for (const dir of fixtures) rmSync(dir, { recursive: true, force: true });
});

const gitIn = (dir) => (args) => {
  const result = spawnSync("git", args, { cwd: dir, encoding: "utf8" });
  assert.equal(result.status, 0, `git ${args.join(" ")}: ${result.stderr}`);
  return result.stdout;
};

const write = (dir, files) => {
  for (const [file, content] of Object.entries(files)) {
    if (content === null) {
      rmSync(join(dir, file), { force: true });
      continue;
    }
    mkdirSync(dirname(join(dir, file)), { recursive: true });
    writeFileSync(join(dir, file), content);
  }
};

/**
 * A repository whose branch `main` holds `base`, and a branch `pr` checked
 * out on top of it with one commit per entry of `commits`: each entry's
 * `files` (null deletes a file) and its `message`.
 */
function repo(base, commits) {
  const dir = mkdtempSync(join(tmpdir(), "check-test-changes-"));
  fixtures.push(dir);
  const git = gitIn(dir);
  git(["init", "--quiet", "--initial-branch=main"]);
  git(["config", "user.email", "test@example.com"]);
  git(["config", "user.name", "Test"]);
  git(["config", "commit.gpgsign", "false"]);
  write(dir, { "README.md": "fixture\n", ...base });
  git(["add", "--all"]);
  git(["commit", "--quiet", "-m", "base"]);
  git(["checkout", "--quiet", "-b", "pr"]);
  for (const { files, message } of commits) {
    write(dir, files);
    git(["add", "--all"]);
    git(["commit", "--quiet", "--allow-empty", "-m", message]);
  }
  return dir;
}

const check = (dir, args = ["--base", "main", "--head", "HEAD"], env = {}) => {
  const result = spawnSync(process.execPath, [script, ...args], {
    cwd: dir,
    encoding: "utf8",
    env: { ...process.env, GITHUB_ACTIONS: "", ...env },
  });
  return { status: result.status, out: result.stdout + result.stderr };
};

/** The pull request from `base` to `head`, one commit with `message`. */
const pr = (base, head, message = "NOC-1: change") =>
  check(repo(base, [{ files: head, message }]));

const rust = (...tests) =>
  `use crate::x;\n\n${tests.join("\n")}`;
const rustTest = (name, body = "assert_eq!(1 + 1, 2);", attrs = "#[test]") =>
  `${attrs}\nfn ${name}() {\n    ${body}\n}\n`;
const ts = (...tests) =>
  `import { expect, it } from "vitest";\n\n${tests.join("\n")}`;
const tsTest = (name, body = "expect(1 + 1).toBe(2);", call = "it") =>
  `${call}("${name}", () => {\n  ${body}\n});\n`;

const RS = "crates/a/tests/a.rs";
const TS = "packages/sdk/test/a.test.ts";

test("a removed Rust #[test] is flagged by name", () => {
  const { status, out } = pr(
    { [RS]: rust(rustTest("keeps"), rustTest("goes", "assert!(true);")) },
    { [RS]: rust(rustTest("keeps")) },
  );
  assert.equal(status, 1, out);
  assert.match(out, /crates\/a\/tests\/a\.rs: removes the test `goes`/);
  assert.doesNotMatch(out, /`keeps`/);
});

test("a removed TypeScript it or test is flagged by name", () => {
  const { status, out } = pr(
    {
      [TS]: ts(tsTest("keeps"), tsTest("goes", "expect(2).toBe(2);", "test")),
      "scripts/b.test.mjs": `test("node goes", () => {\n  assert.ok(1);\n});\n`,
    },
    { [TS]: ts(tsTest("keeps")), "scripts/b.test.mjs": null },
  );
  assert.equal(status, 1, out);
  assert.match(out, /packages\/sdk\/test\/a\.test\.ts: removes the test `goes`/);
  assert.match(out, /scripts\/b\.test\.mjs: removes the test `node goes`/);
  assert.doesNotMatch(out, /`keeps`/);
});

test("a test moved to another file is not flagged", () => {
  const { status, out } = pr(
    { [RS]: rust(rustTest("one"), rustTest("two")) },
    {
      [RS]: rust(rustTest("one")),
      "crates/a/tests/b.rs": rust(rustTest("two")),
    },
  );
  assert.equal(status, 0, out);
});

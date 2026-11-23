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

test("a new #[ignore] on a Rust test is flagged", () => {
  for (const attrs of [
    "#[test]\n#[ignore]",
    '#[tokio::test]\n#[ignore = "slow"]',
    '#[test]\n#[cfg_attr(not(feature = "aws"), ignore)]',
  ]) {
    const { status, out } = pr(
      { [RS]: rust(rustTest("runs")) },
      { [RS]: rust(rustTest("runs", undefined, attrs)) },
    );
    assert.equal(status, 1, `${attrs}\n${out}`);
    assert.match(out, /crates\/a\/tests\/a\.rs: ignores the test `runs`/);
  }
});

test("a new .skip, .only or skip option on a TypeScript test is flagged", () => {
  for (const [head, marker] of [
    [tsTest("runs", undefined, "it.skip"), "it.skip"],
    [tsTest("runs", undefined, "test.only"), "test.only"],
    [tsTest("runs", undefined, "it.skipIf(ci)"), "it.skipIf"],
    [tsTest("runs", undefined, "it.todo"), "it.todo"],
    [`describe.skip("all", () => {\n${tsTest("runs")}});\n`, "describe.skip"],
    [`xit("runs", () => {\n  expect(1 + 1).toBe(2);\n});\n`, "xit"],
    [`it("runs", { skip: true }, () => {\n  expect(1 + 1).toBe(2);\n});\n`, "skip:"],
    [`it("runs", (t) => {\n  t.skip();\n  expect(1 + 1).toBe(2);\n});\n`, "t.skip"],
  ]) {
    const { status, out } = pr({ [TS]: ts(tsTest("runs")) }, { [TS]: ts(head) });
    assert.equal(status, 1, `${marker}\n${out}`);
    assert.ok(
      out.includes(`packages/sdk/test/a.test.ts: adds \`${marker}\``),
      `${marker}\n${out}`,
    );
  }
});

test("an ignored test or a skip marker that was there before is not flagged", () => {
  const ignored = rust(rustTest("slow", undefined, '#[test]\n#[ignore = "slow"]'));
  const skipped = ts(tsTest("later", undefined, "it.skip"));
  const { status, out } = pr(
    { [RS]: ignored, [TS]: skipped },
    { [RS]: `${ignored}\n// touched\n`, [TS]: `${skipped}\n// touched\n` },
  );
  assert.equal(status, 0, out);
});

test("a removed Rust assert or TypeScript expect is flagged", () => {
  const { status, out } = pr(
    {
      [RS]: rust(rustTest("one", "assert!(ready());\n    assert_eq!(count(), 2);")),
      [TS]: ts(tsTest("one", "expect(ready()).toBe(true);\n  expect(count()).toBe(2);")),
      "scripts/b.test.mjs": `test("b", () => {\n  assert.equal(count(), 2);\n  assert.ok(ready());\n});\n`,
    },
    {
      [RS]: rust(rustTest("one", "assert!(ready());")),
      [TS]: ts(tsTest("one", "expect(ready()).toBe(true);")),
      "scripts/b.test.mjs": `test("b", () => {\n  assert.ok(ready());\n});\n`,
    },
  );
  assert.equal(status, 1, out);
  assert.ok(out.includes(`${RS}: removes or rewrites the assertion \`assert_eq!(count(), 2)\``), out);
  assert.ok(out.includes(`${TS}: removes or rewrites the assertion \`expect(count()).toBe(2)\``), out);
  assert.ok(out.includes("scripts/b.test.mjs: removes or rewrites the assertion `assert.equal(count(), 2)`"), out);
  assert.doesNotMatch(out, /ready/);
});

test("an assertion inside a Rust #[cfg(test)] module counts, one outside does not", () => {
  const src = (inner, outer) =>
    `pub fn f(x: u8) {\n    ${outer}\n}\n\n#[cfg(test)]\nmod tests {\n    fn helper() {\n        ${inner}\n    }\n}\n`;
  const kept = pr(
    { "crates/a/src/lib.rs": src("assert_eq!(f(1), ());", "assert!(x > 0);") },
    { "crates/a/src/lib.rs": src("assert_eq!(f(1), ());", "") },
  );
  assert.equal(kept.status, 0, kept.out);
  const removed = pr(
    { "crates/a/src/lib.rs": src("assert_eq!(f(1), ());", "assert!(x > 0);") },
    { "crates/a/src/lib.rs": src("", "assert!(x > 0);") },
  );
  assert.equal(removed.status, 1, removed.out);
  assert.match(removed.out, /removes or rewrites the assertion `assert_eq!\(f\(1\), \(\)\)`/);
});

test("a changed literal in an assertion is flagged with both versions", () => {
  const { status, out } = pr(
    {
      [RS]: rust(rustTest("one", 'assert_eq!(name(), "alpha");')),
      [TS]: ts(tsTest("one", "expect(count()).toBe(2);")),
    },
    {
      [RS]: rust(rustTest("one", 'assert_eq!(name(), "beta");')),
      [TS]: ts(tsTest("one", "expect(count()).toBe(3);")),
    },
  );
  assert.equal(status, 1, out);
  assert.ok(out.includes(`${RS}: changes an expected value: \`assert_eq!(name(), "alpha")\` became \`assert_eq!(name(), "beta")\``), out);
  assert.ok(out.includes(`${TS}: changes an expected value: \`expect(count()).toBe(2)\` became \`expect(count()).toBe(3)\``), out);
});

test("an assertion only reformatted or moved to another file is not flagged", () => {
  const { status, out } = pr(
    {
      [TS]: ts(tsTest("one", "expect(count()).toEqual([1, 2]);")),
      [RS]: rust(rustTest("one", "assert_eq!(count(), 2);")),
    },
    {
      [TS]: ts(tsTest("one", "expect(count()).toEqual([\n    1,\n    2,\n  ]);")),
      [RS]: rust(rustTest("one", "helper();")),
      "crates/a/tests/b.rs": rust("fn helper() {\n    assert_eq!( count(), 2 );\n}\n"),
    },
  );
  assert.equal(status, 0, out);
});

const REMOVED = [
  { [RS]: rust(rustTest("keeps"), rustTest("goes")) },
  { [RS]: rust(rustTest("keeps")) },
];

test("a flagged pull request passes with a Test-change trailer in one of its commits", () => {
  const dir = repo(REMOVED[0], [
    { files: REMOVED[1], message: "NOC-1: drop the goes test\n\nTest-change: goes duplicated keeps since NOC-7" },
    { files: { "README.md": "later\n" }, message: "NOC-1: docs" },
  ]);
  const { status, out } = check(dir);
  assert.equal(status, 0, out);
  assert.match(out, /removes the test `goes`/);
  assert.match(out, /Test-change: goes duplicated keeps since NOC-7 \([0-9a-f]{7}\)/);
});

test("a flagged pull request without the trailer fails and says how to pass", () => {
  const { status, out } = pr(...REMOVED, "NOC-1: drop the goes test");
  assert.equal(status, 1, out);
  assert.match(out, /Test-change: <reason>/);
  assert.match(out, /FACTORY\.md/);
});

test("a trailer that names no reason in words does not count", () => {
  for (const reason of ["", "-", "n/a", "none", "<reason>", "TODO", "fix", "x y"]) {
    const message = `NOC-1: drop\n\nTest-change: ${reason}`;
    const { status, out } = pr(...REMOVED, message);
    assert.equal(status, 1, `${JSON.stringify(reason)}\n${out}`);
  }
});

test("a trailer in a commit of the base branch does not count", () => {
  const dir = repo(REMOVED[0], [{ files: REMOVED[1], message: "NOC-1: drop" }]);
  const git = gitIn(dir);
  git(["checkout", "--quiet", "main"]);
  git(["commit", "--quiet", "--allow-empty", "-m", "NOC-2: other\n\nTest-change: another pull request's reason"]);
  git(["checkout", "--quiet", "pr"]);
  git(["merge", "--quiet", "--no-edit", "main"]);
  const { status, out } = check(dir);
  assert.equal(status, 1, out);
});

test("a pull request that only adds tests passes untouched", () => {
  const { status, out } = pr(
    { [RS]: rust(rustTest("one")), [TS]: ts(tsTest("one")) },
    {
      [RS]: rust(rustTest("one"), rustTest("two", "assert_eq!(two(), 2);")),
      [TS]: ts(tsTest("one"), tsTest("two", "expect(two()).toBe(2);")),
      "crates/a/tests/new.rs": rust(rustTest("three")),
      "packages/sdk/test/new.test.ts": ts(tsTest("three")),
    },
  );
  assert.deepEqual({ status, out }, { status: 0, out: "" });
});

test("tests, markers and assertions in strings and comments do not count", () => {
  const rsBase = rust(
    rustTest("one", 'let s = "#[test] fn gone() {} assert!(x)";\n    // assert_eq!(a, 1);\n    assert_eq!(s.len(), 2);'),
  );
  const rsHead = rust(
    rustTest("one", 'let s = r#"#[ignore] fn x() {} "#;\n    /* assert!(y) */\n    let c = \'(\';\n    assert_eq!(s.len(), 2);'),
  );
  const tsBase = ts(
    tsTest("one", 'const s = "it(\\"ghost\\", () => {})";\n  // expect(a).toBe(1);\n  expect(s.length).toBe(2);'),
  );
  const tsHead = ts(
    tsTest("one", "const s = `it.skip(${\"x\"}) skip: t.skip()`;\n  const r = /[(]\\)/;\n  /* expect(b).toBe(2); */\n  expect(s.length).toBe(2);"),
  );
  const { status, out } = pr({ [RS]: rsBase, [TS]: tsBase }, { [RS]: rsHead, [TS]: tsHead });
  assert.deepEqual({ status, out }, { status: 0, out: "" });
});

test("an assertion with a bracket in a string is read to its real end", () => {
  const { status, out } = pr(
    {
      [RS]: rust(rustTest("one", 'assert_eq!(f(")"), 1);\n    assert!(g());')),
      [TS]: ts(tsTest("one", 'expect(f(")")).toBe(1);\n  expect(g()).toBe(true);')),
    },
    {
      [RS]: rust(rustTest("one", 'assert_eq!(f(")"), 2);\n    assert!(g());')),
      [TS]: ts(tsTest("one", 'expect(f(")")).toBe(2);\n  expect(g()).toBe(true);')),
    },
  );
  assert.equal(status, 1, out);
  assert.ok(out.includes('`assert_eq!(f(")"), 1)` became `assert_eq!(f(")"), 2)`'), out);
  assert.ok(out.includes('`expect(f(")")).toBe(1)` became `expect(f(")")).toBe(2)`'), out);
  assert.doesNotMatch(out, /g\(\)/);
});

test("without --base it compares with origin/main, as pnpm gates runs it", () => {
  const dir = repo(...REMOVED.map((files, i) => (i ? [{ files, message: "NOC-1: drop" }] : files)));
  gitIn(dir)(["update-ref", "refs/remotes/origin/main", "main"]);
  const { status, out } = check(dir, []);
  assert.equal(status, 1, out);
  assert.match(out, /removes the test `goes`/);
});

test("without --base and without origin/main it says it compared nothing, and passes", () => {
  const dir = repo(...REMOVED.map((files, i) => (i ? [{ files, message: "NOC-1: drop" }] : files)));
  const { status, out } = check(dir, []);
  assert.equal(status, 0, out);
  assert.match(out, /no origin\/main/);
  assert.match(out, /the `changes` check/);
});

test("a --base that does not exist is an error, never a pass", () => {
  const dir = repo(...REMOVED.map((files, i) => (i ? [{ files, message: "NOC-1: drop" }] : files)));
  const { status, out } = check(dir, ["--base", "nowhere"]);
  assert.equal(status, 1, out);
  assert.match(out, /nowhere/);
});

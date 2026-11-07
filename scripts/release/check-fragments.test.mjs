import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { GIT_ENV, makeRepo } from "./lib/test-repo.mjs";

const script = join(
  dirname(fileURLToPath(import.meta.url)),
  "check-fragments.mjs",
);
const run = (cwd, ...args) =>
  spawnSync(process.execPath, [script, ...args], { cwd, encoding: "utf8" });
const fixture = (t, files) => {
  const dir = mkdtempSync(join(tmpdir(), "check-fragments-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  for (const [file, text] of Object.entries(files)) {
    mkdirSync(join(dir, dirname(file)), { recursive: true });
    writeFileSync(join(dir, file), text);
  }
  return dir;
};
const ok =
  "---\ntype: feature\ntickets: [mt08.1qaz]\ncomponents: [core]\n---\n## Problem\na\n## Use\nb\n";

test("passes a directory of valid fragments and ignores README.md", (t) => {
  const dir = fixture(t, {
    "changes/add-y.md": ok,
    "changes/README.md": "# guide\n",
  });
  const r = run(dir);
  assert.equal(r.status, 0, r.stderr);
});

test("names the file and the rule of every violation", (t) => {
  const dir = fixture(t, {
    "changes/add-y.md": ok,
    "changes/Bad_Slug.md": ok,
    "changes/no-use.md": ok.replace("## Use\nb\n", ""),
  });
  const r = run(dir);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /check-fragments: changes\/Bad_Slug\.md: slug/);
  assert.match(
    r.stderr,
    /check-fragments: changes\/no-use\.md: missing section ## Use/,
  );
  assert.doesNotMatch(r.stderr, /add-y/);
});

test("a missing changes/ directory passes (nothing to check) and says so", (t) => {
  const dir = fixture(t, {});
  const r = run(dir);
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /no changes\/ directory/);
});

test("--help exits 0 and an unknown flag exits 2", () => {
  const help = run(tmpdir(), "--help");
  assert.equal(help.status, 0);
  assert.match(help.stdout, /^usage: check-fragments/);
  assert.doesNotMatch(help.stdout, /nothing to check/);
  assert.equal(run(tmpdir(), "--bogus").status, 2);
});

const fix =
  "---\ntype: fix\ntickets: [mt08.1qaz]\nintroduced: 0.1.0\ncomponents: [core]\n---\n## Symptom\na\n## When\nb\n## Cause\nc\n## Fix\nd\n";

test("passes valid fragments of every type", (t) => {
  const dir = fixture(t, {
    "changes/a-breaking.md":
      "---\ntype: breaking\ntickets: [mt08.1qaz]\ncomponents: [core]\n---\n## Problem\na\n## Use\nb\n## Migration\nc\n",
    "changes/a-feature.md": ok,
    "changes/a-feature-issues.md": ok.replace(
      "components:",
      "issues: [42, 57]\ncomponents:",
    ),
    "changes/a-fix.md": fix,
    "changes/a-fix-cycle.md": fix.replace("0.1.0", "cycle"),
    "changes/a-deprecation.md":
      "---\ntype: deprecation\ntickets: [mt08.1qaz]\ncomponents: [sdk-ts]\n---\n## Deprecated\na\n## Replacement\nb\n## Removed in\n0.3.0\n",
    "changes/a-security.md":
      "---\ntype: security\ntickets: [mt08.1qaz]\ncomponents: [relay]\n---\n## Description\na\n## Advisory\npending\n",
    "changes/a-internal.md":
      "---\ntype: internal\ntickets: [mt08.1qaz]\ncomponents: [cli, sdk-rust]\n---\nBumped the build.\n",
  });
  const r = run(dir);
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stderr, "");
});

test("exits 1 and names file and rule for every rule of the schema", (t) => {
  const bad = {
    Bad_Slug: [ok, /slug/],
    "no-front": ["## Problem\na\n## Use\nb\n", /no front matter/],
    "unknown-type": [ok.replace("feature", "chore"), /type must be one of/],
    "empty-tickets": [ok.replace("[mt08.1qaz]", "[]"), /tickets/],
    "bad-ticket": [ok.replace("mt08.1qaz", "noc-12"), /tickets/],
    "empty-components": [ok.replace("[core]", "[]"), /components/],
    "unknown-component": [ok.replace("[core]", "[web]"), /components/],
    "missing-section": [
      ok.replace("## Use\nb\n", ""),
      /missing section ## Use/,
    ],
    "unexpected-section": [`${ok}## Notes\nx\n`, /unexpected section ## Notes/],
    "introduced-on-feature": [
      ok.replace("components:", "introduced: cycle\ncomponents:"),
      /introduced is only allowed on fix/,
    ],
    "fix-no-introduced": [
      fix.replace("introduced: 0.1.0\n", ""),
      /fix needs introduced/,
    ],
    "fix-bad-introduced": [
      fix.replace("0.1.0", "v0.1"),
      /fix needs introduced/,
    ],
    "unknown-field": [
      ok.replace("components:", "author: me\ncomponents:"),
      /unknown field author/,
    ],
    "bad-issues": [
      ok.replace("components:", "issues: [42, 0]\ncomponents:"),
      /issues: 0 is not a positive integer/,
    ],
    "internal-section": [
      "---\ntype: internal\ntickets: [mt08.1qaz]\ncomponents: [core]\n---\n## Problem\nx\n",
      /internal has a one-line body/,
    ],
    "internal-two-lines": [
      "---\ntype: internal\ntickets: [mt08.1qaz]\ncomponents: [core]\n---\none\ntwo\n",
      /internal has a one-line body/,
    ],
  };
  const dir = fixture(
    t,
    Object.fromEntries(
      Object.entries(bad).map(([slug, [text]]) => [`changes/${slug}.md`, text]),
    ),
  );
  const r = run(dir);
  assert.equal(r.status, 1);
  const lines = r.stderr.trim().split("\n");
  for (const [slug, [, rule]] of Object.entries(bad)) {
    const prefix = `check-fragments: changes/${slug}.md: `;
    assert.ok(
      lines.some(
        (line) =>
          line.startsWith(prefix) && rule.test(line.slice(prefix.length)),
      ),
      `${slug}: ${r.stderr}`,
    );
  }
});

test("--dir checks another directory", (t) => {
  const dir = fixture(t, {
    "elsewhere/no-use.md": ok.replace("## Use\nb\n", ""),
    "changes/add-y.md": ok,
  });
  const r = run(dir, "--dir", "elsewhere");
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /check-fragments: elsewhere\/no-use\.md: missing section ## Use/,
  );
});

test("--dir naming a file, not a directory, fails with a named message", (t) => {
  const dir = fixture(t, { "notes.md": ok });
  const r = run(dir, "--dir", "notes.md");
  assert.equal(r.status, 1);
  assert.equal(
    r.stderr,
    "check-fragments: --dir notes.md is not a directory\n",
  );
});

test("a fragment path that cannot be read fails naming it, and the others are still checked", (t) => {
  const dir = fixture(t, {
    "changes/a-dir.md/inner.txt": "x\n",
    "changes/no-use.md": ok.replace("## Use\nb\n", ""),
  });
  const r = run(dir);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /^check-fragments: changes\/a-dir\.md: .*EISDIR/m);
  assert.match(
    r.stderr,
    /^check-fragments: changes\/no-use\.md: missing section ## Use$/m,
  );
  assert.doesNotMatch(r.stderr, /\n\s+at /, "a stack trace");
});

test("type: constructor and CRLF line endings fail with a named rule, not a crash", (t) => {
  const dir = fixture(t, {
    "changes/proto.md": ok.replace("type: feature", "type: constructor"),
    "changes/crlf.md": ok.replaceAll("\n", "\r\n"),
  });
  const r = run(dir);
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /^check-fragments: changes\/proto\.md: type must be one of/m,
  );
  assert.match(
    r.stderr,
    /^check-fragments: changes\/crlf\.md: has CRLF line endings; save it with LF line endings$/m,
  );
  assert.doesNotMatch(r.stderr, /\n\s+at /, "a stack trace");
});

test("--dir without a value is a usage error", () => {
  assert.equal(run(tmpdir(), "--dir").status, 2);
});

// PR mode: every test builds a small repository with lib/test-repo.mjs and
// spawns the script in it with GIT_ENV, so no user or system git config leaks in.
const runIn = (repo, ...args) =>
  spawnSync(process.execPath, [script, ...args], {
    cwd: repo.dir,
    encoding: "utf8",
    env: GIT_ENV,
  });
/** The paths the missing-fragment failure names, in the order printed. */
const shipping = (stderr) =>
  stderr
    .split("\n")
    .map((line) => /^check-fragments: (.+) ships to a user/.exec(line)?.[1])
    .filter(Boolean);
const repoFor = (t) => {
  const repo = makeRepo();
  t.after(() => repo.dispose());
  return repo;
};

test("PR mode: a product path without a fragment fails and names the path", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", {
    "README.md": "x\n",
    "crates/c/src/lib.rs": "fn a(){}\n",
  });
  const head = repo.commit("change core", {
    "crates/c/src/lib.rs": "fn b(){}\n",
  });
  const r = runIn(repo, "--base", base, "--head", head);
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /^check-fragments: crates\/c\/src\/lib\.rs ships to a user and the pull request adds no fragment under changes\/ \(add one, or label the PR skip-changes\)$/m,
  );
});

test("PR mode: every product path is named, allowlisted ones are not", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", { "Cargo.toml": "a\n", "docs/a.md": "a\n" });
  const head = repo.commit("both", {
    "Cargo.toml": "b\n",
    "packages/sdk/package.json": "{}\n",
    "docs/a.md": "b\n",
  });
  const r = runIn(repo, "--base", base, "--head", head);
  assert.equal(r.status, 1);
  // The path lines only; the how-to after them has a test of its own.
  const named = shipping(r.stderr);
  assert.deepEqual(named.sort(), ["Cargo.toml", "packages/sdk/package.json"]);
});

test("PR mode: allowlisted paths need no fragment", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", { "README.md": "x\n" });
  const head = repo.commit("docs", {
    "README.md": "y\n",
    "docs/a.md": "a\n",
    "scripts/x.mjs": "",
    ".github/workflows/w.yml": "",
    "packages/sdk/test/a.test.ts": "",
    "moon.yml": "",
  });
  const r = runIn(repo, "--base", base, "--head", head);
  assert.equal(r.status, 0, r.stderr);
});

test("PR mode: a deleted product path counts as a change", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", { "crates/c/src/old.rs": "a\n" });
  repo.remove("crates/c/src/old.rs");
  const head = repo.commit("delete");
  const r = runIn(repo, "--base", base, "--head", head);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /crates\/c\/src\/old\.rs ships to a user/);
});

test("PR mode: path names are read verbatim (-z), not C-quoted", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", { "README.md": "x\n" });
  const head = repo.commit("odd names", {
    "crates/c/src/\u00fcber.rs": "a\n",
    'docs/a "b".md': "a\n",
  });
  const r = runIn(repo, "--base", base, "--head", head);
  assert.equal(r.status, 1);
  assert.deepEqual(shipping(r.stderr), ["crates/c/src/\u00fcber.rs"]);
});

test("PR mode: an added fragment satisfies the rule, and is itself validated", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", { "Cargo.toml": "a\n" });
  const head = repo.commit("feat", {
    "Cargo.toml": "b\n",
    "changes/add-y.md": ok,
  });
  const good = runIn(repo, "--base", base, "--head", head);
  assert.equal(good.status, 0, good.stderr);
  const bad = repo.commit("bad", {
    "changes/add-y.md": ok.replace("## Use\nb\n", ""),
  });
  const r = runIn(repo, "--base", base, "--head", bad);
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /^check-fragments: changes\/add-y\.md: missing section ## Use$/m,
  );
});

test("PR mode: fragments are read at head, not from the working tree", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", { "Cargo.toml": "a\n" });
  const head = repo.commit("feat", {
    "Cargo.toml": "b\n",
    "changes/add-y.md": ok.replace("## Use\nb\n", ""),
  });
  repo.write("changes/add-y.md", ok);
  const r = runIn(repo, "--base", base, "--head", head);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /changes\/add-y\.md: missing section ## Use/);
});

test("PR mode: a modified fragment is validated too", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", { "changes/old.md": ok });
  const head = repo.commit("break it", {
    "changes/old.md": ok.replace("## Use\nb\n", ""),
  });
  const r = runIn(repo, "--base", base, "--head", head);
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /^check-fragments: changes\/old\.md: missing section ## Use$/m,
  );
});

test("PR mode: a deleted fragment is not validated and does not crash", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", { "changes/old.md": ok });
  repo.remove("changes/old.md");
  const head = repo.commit("ship it");
  const r = runIn(repo, "--base", base, "--head", head);
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stderr, "");
});

test("PR mode: --skip waives the required fragment but not --require-breaking", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", { "Cargo.toml": "a\n" });
  const head = repo.commit("bump", { "Cargo.toml": "b\n" });
  const skipped = runIn(repo, "--base", base, "--head", head, "--skip");
  assert.equal(skipped.status, 0, skipped.stderr);
  const r = runIn(
    repo,
    "--base",
    base,
    "--head",
    head,
    "--skip",
    "--require-breaking",
  );
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /^check-fragments: a breaking change was detected and no breaking fragment was added \(the skip-changes label does not waive this\)$/m,
  );
});

test("PR mode: --skip does not waive the validation of a fragment", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", { "Cargo.toml": "a\n" });
  const head = repo.commit("bad", {
    "changes/add-y.md": ok.replace("## Use\nb\n", ""),
  });
  const r = runIn(repo, "--base", base, "--head", head, "--skip");
  assert.equal(r.status, 1);
  assert.match(r.stderr, /changes\/add-y\.md: missing section ## Use/);
});

const breaking =
  "---\ntype: breaking\ntickets: [mt08.1qaz]\ncomponents: [core]\n---\n## Problem\na\n## Use\nb\n## Migration\nc\n";

test("PR mode: --require-breaking passes with a breaking fragment", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", { "Cargo.toml": "a\n" });
  const head = repo.commit("break", {
    "Cargo.toml": "b\n",
    "changes/break-x.md": breaking,
  });
  const r = runIn(repo, "--base", base, "--head", head, "--require-breaking");
  assert.equal(r.status, 0, r.stderr);
});

test("PR mode: --require-breaking is not met by a fragment of another type", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", { "Cargo.toml": "a\n" });
  const head = repo.commit("feat", {
    "Cargo.toml": "b\n",
    "changes/add-y.md": ok,
  });
  const r = runIn(repo, "--base", base, "--head", head, "--require-breaking");
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /breaking change was detected and no breaking fragment/,
  );
});

test("PR mode: --require-breaking is not met by a modified breaking fragment", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", {
    "Cargo.toml": "a\n",
    "changes/break-x.md": breaking,
  });
  const head = repo.commit("edit", {
    "Cargo.toml": "b\n",
    "changes/break-x.md": `${breaking}more\n`,
    "changes/add-y.md": ok,
  });
  const r = runIn(repo, "--base", base, "--head", head, "--require-breaking");
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /breaking change was detected and no breaking fragment/,
  );
});

test("PR mode: the merge base is used, so base-branch changes do not count", (t) => {
  const repo = repoFor(t);
  repo.commit("base", { "README.md": "x\n" });
  repo.run("switch", "-q", "-c", "pr");
  const head = repo.commit("docs", { "docs/a.md": "a\n" });
  repo.run("switch", "-q", "main");
  const main2 = repo.commit("main moves", { "Cargo.toml": "z\n" });
  const r = runIn(repo, "--base", main2, "--head", head);
  assert.equal(r.status, 0, r.stderr);
});

test("PR mode: a fragment added on the base branch does not satisfy the PR", (t) => {
  const repo = repoFor(t);
  repo.commit("base", { "Cargo.toml": "a\n" });
  repo.run("switch", "-q", "-c", "pr");
  const head = repo.commit("feat", { "Cargo.toml": "b\n" });
  repo.run("switch", "-q", "main");
  const main2 = repo.commit("other PR", { "changes/add-y.md": ok });
  const r = runIn(repo, "--base", main2, "--head", head);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /Cargo\.toml ships to a user/);
});

test("PR mode: a modified fragment that was not added does not satisfy the rule", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", {
    "Cargo.toml": "a\n",
    "changes/old.md": ok,
  });
  const head = repo.commit("edit", {
    "Cargo.toml": "b\n",
    "changes/old.md": `${ok}\n`,
  });
  const r = runIn(repo, "--base", base, "--head", head);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /Cargo\.toml ships to a user/);
});

test("PR mode: a file under changes/ that is not a fragment does not satisfy the rule", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", { "Cargo.toml": "a\n" });
  const head = repo.commit("guide", {
    "Cargo.toml": "b\n",
    "changes/README.md": "# guide\n",
  });
  const r = runIn(repo, "--base", base, "--head", head);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /Cargo\.toml ships to a user/);
});

test("PR mode: --base and --head go together, else a usage error", (t) => {
  const repo = repoFor(t);
  const sha = repo.commit("base", { "README.md": "x\n" });
  assert.equal(runIn(repo, "--base", sha).status, 2);
  assert.equal(runIn(repo, "--head", sha).status, 2);
  assert.equal(runIn(repo, "--skip").status, 2);
  assert.equal(runIn(repo, "--require-breaking").status, 2);
  assert.equal(runIn(repo, "--base").status, 2);
});

test("PR mode: an unknown revision fails, naming git's error", (t) => {
  const repo = repoFor(t);
  const sha = repo.commit("base", { "README.md": "x\n" });
  const r = runIn(repo, "--base", "nope", "--head", sha);
  assert.notEqual(r.status, 0);
  assert.match(
    r.stderr,
    /^check-fragments: git merge-base nope [0-9a-f]{40} failed: /m,
  );
});

test("ALLOWLIST is the 21 patterns of epic \u00a712 plus factory/**, verbatim", async () => {
  const { ALLOWLIST } = await import("./check-fragments.mjs");
  assert.deepEqual(ALLOWLIST, [
    "docs/**",
    "README.md",
    "AGENTS.md",
    "CONTRIBUTING.md",
    "NEXUS_MEMORY.md",
    "factory/**",
    "LICENSE-*",
    ".github/**",
    "scripts/**",
    "moon.yml",
    "**/moon.yml",
    ".moon/**",
    "biome.json",
    "tsconfig.base.json",
    ".prototools",
    ".nvmrc",
    ".gitignore",
    "**/tests/**",
    "**/test/**",
    "**/*.test.*",
    "changes/**",
    "release-notes/**",
  ]);
});

test("PR mode: a fragment main deleted after the fork does not count as added by the PR", (t) => {
  const repo = repoFor(t);
  repo.commit("base", { "Cargo.toml": "a\n", "changes/old.md": ok });
  repo.run("switch", "-q", "-c", "pr");
  const head = repo.commit("feat", { "Cargo.toml": "b\n" });
  repo.run("switch", "-q", "main");
  repo.remove("changes/old.md");
  const main2 = repo.commit("post-release cleanup");
  const r = runIn(repo, "--base", main2, "--head", head);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /Cargo\.toml ships to a user/);
});

test("PR mode: moving a product file into an allowlisted path names the old path", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", {
    "crates/c/src/long.rs": "x\n".repeat(50),
    "docs/a.md": "a\n",
  });
  repo.run("mv", "crates/c/src/long.rs", "docs/long.rs");
  const head = repo.commit("move");
  const r = runIn(repo, "--base", base, "--head", head);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /crates\/c\/src\/long\.rs ships to a user/);
});

test("PR mode: renaming an existing fragment does not count as adding one", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", {
    "Cargo.toml": "a\n",
    "changes/old.md": ok,
  });
  repo.run("mv", "changes/old.md", "changes/new.md");
  const head = repo.commit("rename", { "Cargo.toml": "b\n" });
  const r = runIn(repo, "--base", base, "--head", head);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /Cargo\.toml ships to a user/);
});

test("PR mode: --require-breaking with a malformed added fragment reports, never crashes", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", { "Cargo.toml": "a\n" });
  const head = repo.commit("bad", {
    "Cargo.toml": "b\n",
    "changes/add-y.md": "no front matter\n",
  });
  const r = runIn(repo, "--base", base, "--head", head, "--require-breaking");
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /^check-fragments: changes\/add-y\.md: no front matter/m,
  );
  assert.match(
    r.stderr,
    /breaking change was detected and no breaking fragment/,
  );
  assert.doesNotMatch(r.stderr, /at .*\.mjs:\d+/);
});

test("PR mode: the missing-fragment failure says once, after the paths, how to add one", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", {
    "Cargo.toml": "a\n",
    "Cargo.lock": "a\n",
  });
  const head = repo.commit("change", {
    "Cargo.toml": "b\n",
    "Cargo.lock": "b\n",
  });
  const r = runIn(repo, "--base", base, "--head", head);
  assert.equal(r.status, 1);
  const lines = r.stderr.trimEnd().split("\n");
  const howTo = lines.findIndex((line) =>
    line.startsWith(
      "check-fragments: to add a fragment, create changes/<slug>.md",
    ),
  );
  assert.notEqual(howTo, -1, r.stderr);
  assert.equal(
    lines.filter((line) => line.includes("to add a fragment")).length,
    1,
  );
  for (const path of ["Cargo.toml", "Cargo.lock"]) {
    const named = lines.findIndex((line) =>
      line.startsWith(`check-fragments: ${path} ships to a user`),
    );
    assert.ok(named !== -1 && named < howTo, `${path} after the how-to`);
  }
  const text = lines.slice(howTo).join("\n");
  assert.ok(text.includes("^[a-z0-9][a-z0-9-]*$"), "the slug rule");
  for (const field of ["type:", "tickets:", "issues:", "components:"])
    assert.ok(text.includes(`  ${field}`), field);
  assert.ok(
    text.includes("breaking, feature, fix, deprecation, security, internal"),
    "the types",
  );
  assert.ok(
    text.includes("core, sdk-rust, sdk-ts, cli, relay"),
    "the components",
  );
  assert.ok(text.includes("changes/README.md"), "the writer's guide");
  assert.ok(
    text.includes("docs/release-management.md#writing-a-change-fragment"),
    "the full rules",
  );
});

test("PR mode: no how-to when only a fragment is invalid", (t) => {
  const repo = repoFor(t);
  const base = repo.commit("base", { "README.md": "x\n" });
  const head = repo.commit("bad fragment", {
    "changes/add-y.md": ok.replace("## Use\nb\n", ""),
  });
  const r = runIn(repo, "--base", base, "--head", head);
  assert.equal(r.status, 1);
  assert.doesNotMatch(r.stderr, /to add a fragment/);
});

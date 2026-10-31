import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

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
    "bad-ticket": [ok.replace("mt08.1qaz", "MT08-1"), /tickets/],
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

test("--dir without a value is a usage error", () => {
  assert.equal(run(tmpdir(), "--dir").status, 2);
});

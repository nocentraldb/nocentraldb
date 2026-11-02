import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { validateIndex } from "./index.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const script = join(here, "index.mjs");
const committed = join(here, "..", "..", "release-notes", "index.json");

const entry = (version, stage, tag, date = "2026-03-01") => ({
  version,
  stage,
  tag,
  date,
  path: `release-notes/${version}.md`,
});
const BETA = entry("0.1.0", "beta", "v0.1.0-beta.1");
const RELEASE = entry("0.1.0", "release", "v0.1.0");
const NEXT_BETA = entry("0.1.1", "beta", "v0.1.1-beta.1");

const EMPTY = '{\n  "releases": []\n}\n';

/** A temporary directory holding index.json with `text`. */
const indexFile = (t, text = EMPTY) => {
  const dir = mkdtempSync(join(tmpdir(), "release-index-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const file = join(dir, "index.json");
  writeFileSync(file, text);
  return file;
};

const add = (file, { version, stage, tag, date }) =>
  spawnSync(
    process.execPath,
    [
      script,
      "add",
      "--version",
      version,
      "--stage",
      stage,
      "--tag",
      tag,
      "--date",
      date,
      "--file",
      file,
    ],
    { encoding: "utf8" },
  );

const read = (file) => JSON.parse(readFileSync(file, "utf8"));

test("the committed release-notes/index.json passes validateIndex", () => {
  const index = JSON.parse(readFileSync(committed, "utf8"));
  assert.deepEqual(validateIndex(index), []);
});

test("validateIndex accepts a sorted index of betas and releases", () => {
  assert.deepEqual(
    validateIndex({
      releases: [
        BETA,
        entry("0.1.0", "beta", "v0.1.0-beta.2"),
        RELEASE,
        NEXT_BETA,
      ],
    }),
    [],
  );
});

test("validateIndex rejects a document that is not { releases: [...] }", () => {
  for (const bad of [
    null,
    [],
    "x",
    {},
    { releases: {} },
    { releases: [], extra: 1 },
  ])
    assert.notDeepEqual(validateIndex(bad), [], JSON.stringify(bad));
});

test("validateIndex rejects an entry with an extra or a missing key, naming it", () => {
  const extra = validateIndex({ releases: [{ ...BETA, notes: "x" }] });
  assert.ok(
    extra.some((e) => /v0\.1\.0-beta\.1/.test(e) && /notes/.test(e)),
    extra.join("\n"),
  );
  const { path: _, ...missing } = BETA;
  const errors = validateIndex({ releases: [missing] });
  assert.ok(
    errors.some((e) => /path/.test(e)),
    errors.join("\n"),
  );
});

test("validateIndex rejects a version that is not plain X.Y.Z", () => {
  assert.notDeepEqual(
    validateIndex({
      releases: [{ ...BETA, version: "0.1.0-beta.1" }],
    }),
    [],
  );
});

test("validateIndex rejects bad dates: not YYYY-MM-DD, not a calendar day, a timestamp", () => {
  for (const date of [
    "2026-3-01",
    "2026-02-30",
    "2026-13-01",
    "2026-03-01T10:00:00Z",
    "",
  ]) {
    const errors = validateIndex({ releases: [{ ...BETA, date }] });
    assert.ok(
      errors.some((e) => /date/.test(e)),
      `${date}: ${errors.join("\n")}`,
    );
  }
});

test("validateIndex rejects a path that is not release-notes/<version>.md", () => {
  const errors = validateIndex({
    releases: [{ ...BETA, path: "release-notes/0.1.1.md" }],
  });
  assert.ok(
    errors.some((e) => /path/.test(e)),
    errors.join("\n"),
  );
});

test("validateIndex rejects stage/tag mismatches", () => {
  for (const bad of [
    { ...RELEASE, tag: "v0.1.0-beta.1" },
    { ...BETA, tag: "v0.1.0" },
    { ...BETA, tag: "v0.1.1-beta.1" },
    { ...BETA, tag: "v0.1.0-alpha.1" },
    { ...BETA, stage: "alpha", tag: "v0.1.0-alpha.1" },
  ]) {
    assert.notDeepEqual(
      validateIndex({ releases: [bad] }),
      [],
      JSON.stringify(bad),
    );
  }
});

test("validateIndex rejects a duplicate tag", () => {
  const errors = validateIndex({ releases: [BETA, { ...BETA }] });
  assert.ok(
    errors.some((e) => /v0\.1\.0-beta\.1/.test(e) && /more than once/.test(e)),
    errors.join("\n"),
  );
});

test("validateIndex rejects unsorted entries", () => {
  const errors = validateIndex({ releases: [RELEASE, BETA] });
  assert.ok(
    errors.some((e) => /sorted/.test(e)),
    errors.join("\n"),
  );
});

test("add appends and keeps the entries sorted, whatever the insertion order", (t) => {
  const file = indexFile(t);
  for (const e of [NEXT_BETA, RELEASE, BETA]) {
    const r = add(file, e);
    assert.equal(r.status, 0, r.stderr);
    assert.equal(r.stderr, "");
  }
  assert.deepEqual(read(file), { releases: [BETA, RELEASE, NEXT_BETA] });
  assert.equal(
    readFileSync(file, "utf8"),
    `${JSON.stringify({ releases: [BETA, RELEASE, NEXT_BETA] }, null, 2)}\n`,
  );
});

test("add of an identical entry prints already indexed and leaves the file bytes unchanged", (t) => {
  const file = indexFile(t);
  assert.equal(add(file, BETA).status, 0);
  // Rewritten in a different layout, so a rewrite would show.
  writeFileSync(file, JSON.stringify(read(file)));
  const before = readFileSync(file, "utf8");
  const r = add(file, BETA);
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout, "index: v0.1.0-beta.1 is already indexed\n");
  assert.equal(readFileSync(file, "utf8"), before);
});

test("add of the same tag with a different date fails and writes nothing", (t) => {
  const file = indexFile(t);
  assert.equal(add(file, BETA).status, 0);
  const before = readFileSync(file, "utf8");
  const r = add(file, { ...BETA, date: "2026-03-02" });
  assert.equal(r.status, 1);
  assert.equal(r.stderr, "index: index entry for v0.1.0-beta.1 differs\n");
  assert.equal(readFileSync(file, "utf8"), before);
});

test("add refuses a tag whose stage or version contradicts --stage/--version", (t) => {
  const file = indexFile(t);
  for (const bad of [
    { ...RELEASE, tag: "v0.1.0-beta.1" },
    { ...BETA, tag: "v0.1.0" },
    { ...BETA, version: "0.1.1" },
    { ...BETA, stage: "alpha", tag: "v0.1.0-alpha.1" },
  ]) {
    const r = add(file, bad);
    assert.equal(r.status, 1, JSON.stringify(bad));
    assert.match(r.stderr, /^index: /, JSON.stringify(bad));
  }
  assert.equal(readFileSync(file, "utf8"), EMPTY);
});

test("add refuses a bad date", (t) => {
  const file = indexFile(t);
  const r = add(file, { ...BETA, date: "2026-02-30" });
  assert.equal(r.status, 1);
  assert.match(r.stderr, /^index: .*date/);
  assert.equal(readFileSync(file, "utf8"), EMPTY);
});

test("add refuses an index file that is missing or already invalid, naming it", (t) => {
  const missing = join(dirname(indexFile(t)), "nope.json");
  const r1 = add(missing, BETA);
  assert.equal(r1.status, 1);
  assert.match(r1.stderr, /^index: .*nope\.json/);
  const invalid = indexFile(t, '{ "releases": [], "x": 1 }\n');
  const r2 = add(invalid, BETA);
  assert.equal(r2.status, 1);
  assert.match(r2.stderr, /^index: .*index\.json/);
});

test("--help prints the usage; anything but add with all four options is a usage error", (t) => {
  const file = indexFile(t);
  const help = spawnSync(process.execPath, [script, "--help"], {
    encoding: "utf8",
  });
  assert.equal(help.status, 0);
  assert.match(help.stdout, /^usage: index/);
  for (const args of [
    [],
    ["remove", "--version", "0.1.0"],
    ["add", "--version", "0.1.0", "--stage", "beta", "--tag", "v0.1.0-beta.1"],
    ["add", "--version", "0.1.0", "--stage", "beta", "--file", file],
  ]) {
    const r = spawnSync(process.execPath, [script, ...args], {
      encoding: "utf8",
    });
    assert.equal(r.status, 2, args.join(" "));
    assert.match(r.stderr, /^usage: index/, args.join(" "));
  }
});

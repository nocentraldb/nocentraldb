// The texts' layout (NOC-512): one file per text, texts/<kind>/<name>.json,
// so two pull requests that each add a text add two files and never edit the
// same lines; and the generated modules list their texts in an order that
// keeps two new texts apart, even when their ids are neighbours.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { generate, generateCli, review } from "./texts.mjs";
import { generateCore } from "./texts-core.mjs";
import { entryPath, readTexts, readTextsAt } from "./texts-files.mjs";
import { generateTs } from "./texts-ts.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

const ENTRY = {
  id: "error.noSuchSpace",
  kind: "error",
  code: "notFound",
  text: "no such space",
  when: "A call names a space this database does not hold.",
};

const temp = () => mkdtempSync(join(tmpdir(), "texts-files-"));

const put = (dir, path, content) => {
  mkdirSync(dirname(join(dir, path)), { recursive: true });
  writeFileSync(join(dir, path), content);
};

const json = (entry) => `${JSON.stringify(entry, null, 2)}\n`;

test("a text's file is texts/<kind>/<name>.json", () => {
  assert.equal(entryPath(ENTRY), "texts/error/noSuchSpace.json");
  assert.equal(
    entryPath({ id: "core.viewCorrupt", kind: "core" }),
    "texts/core/viewCorrupt.json",
  );
});

test("every text of the repository is a file of its own, and texts.json is gone", () => {
  assert.ok(!existsSync(join(ROOT, "texts", "texts.json")));
  const { texts, problems } = readTexts(ROOT);
  assert.deepEqual(problems, []);
  assert.ok(texts.length > 400, `${texts.length} texts`);
  const ids = texts.map((t) => t.id);
  assert.deepEqual(ids, [...ids].sort(), "readTexts answers them by id");
  for (const kind of readdirSync(join(ROOT, "texts"))) {
    if (kind === "README.md") continue;
    for (const name of readdirSync(join(ROOT, "texts", kind))) {
      const path = `texts/${kind}/${name}`;
      const entry = JSON.parse(readFileSync(join(ROOT, path), "utf8"));
      assert.equal(entryPath(entry), path);
    }
  }
});

test("a text's file holds one entry, at its id's path, one field per line", () => {
  const dir = temp();
  try {
    put(dir, "texts/README.md", "The texts.\n");
    put(dir, entryPath(ENTRY), json(ENTRY));
    assert.deepEqual(readTexts(dir), { texts: [ENTRY], problems: [] });

    put(dir, "texts/error/elsewhere.json", json({ ...ENTRY, id: "error.x" }));
    put(dir, "texts/part/squeezed.json", `${JSON.stringify(ENTRY)}\n`);
    put(dir, "texts/part/broken.json", "{");
    put(dir, "texts/part/stray.txt", "x");
    put(dir, "texts/loose.json", json(ENTRY));
    const { problems } = readTexts(dir);
    const about = (path) => problems.filter((p) => p.startsWith(path));
    assert.equal(about("texts/error/elsewhere.json").length, 1, problems);
    assert.match(
      about("texts/error/elsewhere.json")[0],
      /belongs in texts\/error\/x\.json/,
    );
    assert.ok(
      about("texts/part/squeezed.json").some((p) =>
        p.includes("JSON.stringify(entry, null, 2)"),
      ),
      problems,
    );
    assert.equal(about("texts/part/broken.json").length, 1, problems);
    assert.equal(about("texts/part/stray.txt").length, 1, problems);
    assert.equal(about("texts/loose.json").length, 1, problems);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

/** `git merge-file` of three versions: the number of conflicts and the result. */
const merge3 = (dir, base, ours, theirs) => {
  for (const [name, content] of Object.entries({ base, ours, theirs }))
    writeFileSync(join(dir, name), content);
  const run = spawnSync(
    "git",
    ["merge-file", "-p", "--quiet", "ours", "base", "theirs"],
    { cwd: dir, encoding: "utf8" },
  );
  return { conflicts: run.status, merged: run.stdout };
};

/**
 * Pairs of new texts of `kind` whose ids are neighbours, nothing sorting
 * between them: `<name>Merge1` and `<name>Merge2` after each of the first
 * `count` texts of the kind, as two pull requests on one feature add them.
 */
const neighbourPairs = (texts, kind, count) =>
  texts
    .filter((t) => t.kind === kind)
    .slice(0, count)
    .map((t) => {
      const pair = ["1", "2"].map((x) => ({
        ...t,
        id: `${t.id}Merge${x}`,
        text: `the text ${x}`,
      }));
      const [a, b] = pair.map((p) => p.id);
      assert.ok(!texts.some((u) => u.id > a && u.id < b), a);
      return pair;
    });

test("two texts added in parallel, neighbours by id, merge without a conflict, in the texts and nearly always in the generated modules", () => {
  const { texts } = readTexts(ROOT);
  const dir = temp();
  try {
    for (const [kind, gen] of [
      ["error", generate],
      ["cli", (t) => ({ "texts.rs": generateCli(t) })],
      ["ts", (t) => ({ "texts.ts": generateTs(t) })],
      ["core", generateCore],
    ]) {
      const pairs = neighbourPairs(texts, kind, 25);
      const base = gen(texts);
      let conflicted = 0;
      for (const [a, b] of pairs) {
        // The texts: two new files, at different paths, and nothing else.
        assert.notEqual(entryPath(a), entryPath(b));
        for (const t of [a, b])
          assert.ok(!existsSync(join(ROOT, entryPath(t))), entryPath(t));
        // The generated modules: each merges by itself to what the merged
        // texts generate, unless the two land in the same gap by chance.
        const ours = gen([...texts, a]);
        const theirs = gen([...texts, b]);
        const both = gen([...texts, a, b]);
        let clean = true;
        for (const name of Object.keys(both)) {
          const merged = merge3(dir, base[name], ours[name], theirs[name]);
          if (merged.conflicts !== 0) clean = false;
          else assert.equal(merged.merged, both[name], `${a.id}: ${name}`);
        }
        if (!clean) conflicted++;
      }
      // In id order every pair conflicts; in hash order a pair lands in the
      // same gap about once in as many texts as the module holds.
      assert.ok(
        conflicted <= Math.ceil(pairs.length / 10),
        `${kind}: ${conflicted} of ${pairs.length} pairs conflict`,
      );
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("a generated module lists its texts in the same order however they are read", () => {
  const { texts } = readTexts(ROOT);
  const errors = texts.filter((t) => t.kind === "error");
  assert.deepEqual(generate([...errors].reverse()), generate(errors));
});

test("the texts at a tag are read from its folder, or from texts.json before the split", () => {
  const dir = temp();
  try {
    const git = (...args) => {
      const run = spawnSync(
        "git",
        ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@t", ...args],
        { encoding: "utf8" },
      );
      assert.equal(run.status, 0, run.stderr);
    };
    const commit = () => {
      git("add", "-A");
      git("commit", "-q", "-m", "texts");
    };
    git("init", "-q");
    put(dir, "README", "x");
    commit();
    git("tag", "v0.1.0");
    assert.deepEqual(readTextsAt(dir, "v0.1.0"), []);

    put(dir, "texts/texts.json", json({ texts: [ENTRY] }));
    commit();
    git("tag", "v0.2.0-beta.1");
    assert.deepEqual(readTextsAt(dir, "v0.2.0-beta.1"), [ENTRY]);

    rmSync(join(dir, "texts", "texts.json"));
    const changed = { ...ENTRY, text: "this database holds no such space" };
    const part = { id: "part.why", kind: "part", text: "why", when: "Always." };
    put(dir, entryPath(changed), json(changed));
    put(dir, entryPath(part), json(part));
    put(dir, "texts/README.md", "The texts.\n");
    commit();
    git("tag", "v0.2.0");
    assert.deepEqual(readTextsAt(dir, "v0.2.0"), [changed, part]);

    const text = review(dir);
    assert.match(text, /^# Texts new or changed since v0\.2\.0$/m);
    assert.ok(text.includes("0 new, 0 changed, 0 removed, of 2 texts"), text);
    const since = review(dir, "v0.2.0-beta.1");
    assert.ok(since.includes("1 new, 1 changed, 0 removed"), since);
    assert.ok(since.includes(`- text now: "${changed.text}"`), since);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// Tests of how scripts/texts.mjs splits the generated modules (NOC-541): the
// Rust SDK's error texts go into several files by a hash of their ids, behind
// one module that re-exports them, so code still names `texts::error::NAME`;
// and the gate fails when a generated module would pass the repository's
// limit for a file, before `repo:file-size` does.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import {
  constName,
  generate,
  generatedModules,
  PATHS,
  SHARDS,
  shardOf,
  tooLong,
} from "./texts.mjs";
import { readTexts } from "./texts-files.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

const error = (name) => ({
  id: `error.${name}`,
  kind: "error",
  code: "invalidArgument",
  text: `The text of ${name}.`,
  when: `${name} fails.`,
});

const shardFiles = (files) =>
  Object.keys(files).filter((n) => /^error\/shard_\d+\.rs$/.test(n));

test("the error texts are split into SHARDS.error files behind error/mod.rs", () => {
  const texts = Array.from({ length: 40 }, (_, i) => error(`text${i}`));
  const files = generate(texts);
  assert.ok(!("error.rs" in files), "no single error.rs");
  assert.ok(SHARDS.error >= 2, `SHARDS.error is ${SHARDS.error}`);
  const shards = shardFiles(files).sort();
  assert.equal(shards.length, SHARDS.error, shards.join());
  const mod = files["error/mod.rs"];
  for (const shard of shards) {
    const name = shard.slice("error/".length, -".rs".length);
    assert.match(
      mod,
      new RegExp(`\\nmod ${name};\\npub\\(crate\\) use ${name}::\\*;`),
    );
  }
  // Each text stands in exactly one file, the one its id's hash names.
  for (const t of texts) {
    const item = `pub(crate) const ${constName(t.id.slice(6))}: &str`;
    const holding = shards.filter((s) => files[s].includes(`${item} =`));
    assert.deepEqual(holding, [`error/shard_${shardOf(t)}.rs`], t.id);
  }
  // The parent module still declares `error`, so the paths stay the same.
  assert.match(files["mod.rs"], /\npub\(crate\) mod error;\n/);
});

test("a text's file depends only on its own id", () => {
  const before = Array.from({ length: 20 }, (_, i) => error(`a${i}`));
  const after = [...before, error("brandNew"), error("another")];
  for (const t of before) assert.equal(shardOf(t), shardOf({ ...t }));
  const where = (texts) => {
    const files = generate(texts);
    return Object.fromEntries(
      texts.map((t) => [
        t.id,
        shardFiles(files).find((s) =>
          files[s].includes(` ${constName(t.id.slice(6))}: &str`),
        ),
      ]),
    );
  };
  const was = where(before);
  const now = where(after);
  for (const t of before) assert.equal(now[t.id], was[t.id], t.id);
});

test("a shard without texts is still written, so the files do not come and go", () => {
  const files = generate([error("only")]);
  assert.equal(shardFiles(files).length, SHARDS.error);
});

test("tooLong names a generated file over the limit, and none at it", () => {
  const lines = (n) => "x\n".repeat(n);
  assert.deepEqual(tooLong({ "a.rs": lines(700), "b.rs": lines(3) }, 700), []);
  const problems = tooLong({ "a.rs": lines(701) }, 700);
  assert.equal(problems.length, 1);
  assert.match(problems[0], /^a\.rs would have 701 lines; the limit is 700/);
});

test("generatedModules holds every generated text file of the repository", () => {
  const texts = readTexts(ROOT).texts;
  const paths = Object.keys(generatedModules(texts));
  for (const expected of [
    `${PATHS.module}/mod.rs`,
    `${PATHS.module}/error/mod.rs`,
    `${PATHS.module}/error/shard_0.rs`,
    `${PATHS.module}/part.rs`,
    PATHS.cliModule,
    "packages/sdk/src/texts.ts",
    "crates/ncdb-core/src/texts/mod.rs",
    "crates/ncdb-core/src/texts/lock.rs",
  ])
    assert.ok(paths.includes(expected), expected);
  for (const [path, content] of Object.entries(generatedModules(texts)))
    assert.equal(readFileSync(join(ROOT, path), "utf8"), content, path);
});

test("the gate fails when a generated module would pass the limit", () => {
  const many = Array.from({ length: 400 }, (_, i) => ({
    id: `part.piece${i}`,
    kind: "part",
    text: `Piece ${i}.`,
    when: `Piece ${i} goes into a message.`,
  }));
  const problems = tooLong(generatedModules(many), 700);
  assert.ok(
    problems.some((p) => p.startsWith(`${PATHS.module}/part.rs would have`)),
    problems.join("\n"),
  );
});

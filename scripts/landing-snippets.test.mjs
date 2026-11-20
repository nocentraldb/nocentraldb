// The landing page's TypeScript snippets type-check against the SDK's source
// (NOC-371), as code-rust.md's Rust ones compile in the doc tests of
// nocentraldb. A step's block is a module of its own, model.ts or <step>.ts,
// and lines that start with "# " compile but stay off the page.
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { check, sections } from "./landing-snippets.mjs";

const MODEL = [
  "## model",
  "",
  "```ts",
  'import { nc } from "@nocentraldb/sdk";',
  "",
  "export const model = nc.model({",
  "  tasks: nc.collection({ title: nc.string() }),",
  "});",
  "```",
];

/** The open step, its setup hidden, with `call` as its one shown line. */
const step = (call) => [
  "## open",
  "",
  "```ts",
  '# import { nc } from "@nocentraldb/sdk";',
  '# import { model } from "./model";',
  '# import lock from "./model.lock.json";',
  "#",
  "const db = await nc.open({ model, lock });",
  call,
  "```",
];

/** Checks a page made of `lines`, in a file of its own. */
const checked = (lines) => {
  const dir = mkdtempSync(join(tmpdir(), "landing-snippets-test-"));
  try {
    const path = join(dir, "code-ts.md");
    writeFileSync(path, `${["# Code", "", ...lines].join("\n")}\n`);
    return check(path);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
};

test("each step's block type-checks as a module, its hidden setup included", () => {
  const result = checked([
    ...MODEL,
    "",
    ...step("const space = await db.spaces.create();"),
  ]);
  assert.ok(result.ok, result.output);
  assert.equal(result.count, 2);
});

test("a wrong call fails at its line in the page", () => {
  const result = checked([...MODEL, "", ...step("db.noSuchCall();")]);
  assert.equal(result.ok, false);
  // the page's line 21 is the wrong call
  assert.match(result.output, /code-ts\.md:21:\d+/);
  assert.match(result.output, /noSuchCall/);
});

test("the shown code of a step leaves out its hidden lines", () => {
  const found = sections(
    ["# Code", "", ...MODEL, "", ...step("db.close();")].join("\n"),
  );
  assert.deepEqual(
    found.map((s) => s.id),
    ["model", "open"],
  );
  assert.equal(
    found[1].shown,
    "const db = await nc.open({ model, lock });\ndb.close();",
  );
});

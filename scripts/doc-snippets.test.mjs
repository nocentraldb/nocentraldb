// Tests of scripts/doc-snippets.mjs, the type check of the browser guide's
// snippets (NOC-322): the guide itself passes, and a guide whose snippet
// leaves the SDK's public API fails at the snippet's line in the guide.
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { check, GUIDE, snippets } from "./doc-snippets.mjs";

const MODEL = [
  "```ts model.ts",
  'import { nc } from "@nocentraldb/sdk";',
  "",
  "export const model = nc.model({",
  "  tasks: nc.collection(",
  '    { title: nc.string(), status: nc.oneOf(["open", "done"]) },',
  '    { indexes: { byStatus: nc.index(["status", "title"], { equal: 1 }) } },',
  "  ),",
  "});",
  "```",
].join("\n");

/** Checks a guide made of `markdown`, in a file of its own. */
const checked = (markdown) => {
  const dir = mkdtempSync(join(tmpdir(), "doc-snippets-test-"));
  try {
    const path = join(dir, "guide.md");
    writeFileSync(path, markdown);
    return check(path);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
};

/** A guide with the model and one snippet of `code`. */
const guide = (code) =>
  [
    "# A guide",
    "",
    MODEL,
    "",
    "Text.",
    "",
    "```ts",
    'import { nc } from "@nocentraldb/sdk";',
    'import { model } from "./model.ts";',
    "",
    "const lock: unknown = await (await fetch('/model.lock.json')).json();",
    'const db = await nc.open({ model, lock, name: "tasks" });',
    code,
    "```",
    "",
  ].join("\n");

test("snippets: each ts block, named by its info string or numbered, with the line its code starts on", () => {
  const found = snippets(
    [
      "```ts model.ts",
      "a",
      "```",
      "",
      "```sh",
      "pnpm install",
      "```",
      "```ts",
      "b",
      "c",
      "```",
    ].join("\n"),
  );
  assert.deepEqual(found, [
    { file: "model.ts", line: 2, code: "a" },
    { file: "snippet-2.ts", line: 9, code: "b\nc" },
  ]);
});

test("snippets: refuses an unclosed block, a name given twice and a name that is no file", () => {
  assert.throws(() => snippets("```ts\na\n"), /never closed/);
  assert.throws(
    () => snippets("```ts a.ts\n```\n```ts a.ts\n```\n"),
    /named twice/,
  );
  assert.throws(() => snippets("```ts ../a.ts\n```\n"), /not a file name/);
});

test("a snippet on the SDK's public API type-checks", () => {
  const result = checked(
    guide(
      [
        "const project = await db.spaces.create();",
        'await project.create(model.tasks, { title: "Milk", status: "open" });',
        'const page = await db.find(model.tasks.indexes.byStatus.equal("open").take(20));',
        "console.log(page.items.length, db.sync.current().connection);",
      ].join("\n"),
    ),
  );
  assert.equal(result.ok, true, result.output);
});

test("a snippet that calls what the SDK lacks fails at its line in the guide", () => {
  const markdown = guide("await db.account.recoveryCode();");
  const line = markdown.split("\n").indexOf("await db.account.recoveryCode();");
  const result = checked(markdown);
  assert.equal(result.ok, false);
  assert.match(result.output, new RegExp(`guide\\.md:${line + 1}:\\d+`));
  assert.match(result.output, /account/);
});

test("a snippet that writes a value its field does not take fails", () => {
  const result = checked(
    guide(
      "await (await db.spaces.create()).create(model.tasks, { title: 1 });",
    ),
  );
  assert.equal(result.ok, false);
  assert.match(result.output, /guide\.md:\d+:\d+/);
});

test("a query on an index the model does not declare fails", () => {
  const result = checked(
    guide("await db.find(model.tasks.indexes.byTitle.take(20));"),
  );
  assert.equal(result.ok, false);
});

test("a guide without a ts snippet fails", () => {
  const result = checked("# A guide\n\n```sh\npnpm install\n```\n");
  assert.equal(result.ok, false);
  assert.match(result.output, /no ts snippet/);
});

test("the browser guide's snippets type-check against the SDK", () => {
  const result = check(GUIDE);
  assert.equal(result.ok, true, result.output);
});

test("the browser guide shows each step the ticket names, in its snippets", () => {
  const code = snippets(readFileSync(GUIDE, "utf8"))
    .map((s) => s.code)
    .join("\n");
  for (const name of [
    "nc.model(",
    "nc.open(",
    "relay:",
    "spaces.create()",
    ".write(",
    ".get(model.tasks",
    "db.find(",
    "db.subscribe(",
    "db.invitation()",
    "nc.invitation(",
    "members.add(",
    "db.sync.subscribe(",
    "db.sync.confirmed(",
  ]) {
    assert.ok(code.includes(name), `no snippet shows ${name}`);
  }
});

// The TypeScript examples of the README and of the docs' Getting started page
// type-check against the SDK's source (NOC-361), as the browser guide's do
// (doc-snippets.mjs). Their blocks read as one app: the block with nc.model
// is the model, and the others continue one another, so checkPage joins them
// into one module, with the names the page leaves to the reader declared.
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { checkPage } from "./doc-snippets.mjs";

const MODEL = [
  "```ts",
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

const OPEN = [
  "```ts",
  'import { nc } from "@nocentraldb/sdk";',
  'import { model } from "./model";',
  'import lock from "./model.lock.json";',
  "",
  "const db = await nc.open({ model, lock });",
  "```",
].join("\n");

/** Checks a page made of `markdown`, in a file of its own. */
const checked = (markdown, options) => {
  const dir = mkdtempSync(join(tmpdir(), "page-snippets-test-"));
  try {
    const path = join(dir, "page.md");
    writeFileSync(path, markdown);
    return checkPage(path, options);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
};

test("blocks that continue one another type-check as one app", () => {
  const page = [
    "# A page",
    MODEL,
    OPEN,
    "```ts",
    "const space = await db.spaces.create();",
    "```",
    "```ts",
    'await space.write((tx) => { tx.create(model.tasks, { title: "x", status: "open" }); });',
    "show(space.id);",
    "```",
  ].join("\n");
  const result = checked(page, {
    declare: ["declare function show(id: string): void;"],
  });
  assert.ok(result.ok, result.output);
  assert.equal(result.count, 4);
});

test("a wrong call fails at its line in the page", () => {
  const page = [
    "# A page",
    MODEL,
    OPEN,
    "```ts",
    "db.noSuchCall();",
    "```",
  ].join("\n");
  const result = checked(page);
  assert.equal(result.ok, false);
  assert.match(result.output, /page\.md:20:\d+/);
});

test("a page without a model block is refused", () => {
  const result = checked(["# A page", OPEN].join("\n"));
  assert.equal(result.ok, false);
  assert.match(result.output, /model/);
});

test("the README's TypeScript examples type-check against the SDK", () => {
  const { ok, output, count } = checkPage("README.md", {
    declare: [
      "declare function send(text: string): void;",
      "declare function render(page: unknown): void;",
      "declare const text: string;",
      "declare const memberId: string;",
    ],
  });
  assert.ok(ok, output);
  assert.ok(count >= 6, `only ${count} snippets`);
});

test("the Getting started page's TypeScript examples type-check against the SDK", () => {
  const { ok, output, count } = checkPage(
    "site/src/content/docs/docs/index.md",
  );
  assert.ok(ok, output);
  assert.ok(count >= 2, `only ${count} snippets`);
});

test("browser durability's example type-checks against the SDK, with the example app's model", () => {
  const { ok, output, count } = checkPage("docs/browser-durability.md", {
    model: readFileSync("apps/tasks-ts/model.ts", "utf8"),
    declare: ["declare function showBanner(durability: unknown): void;"],
  });
  assert.ok(ok, output);
  assert.equal(count, 1);
});

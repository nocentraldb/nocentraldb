// The TypeScript API reference (NOC-361, pm's option A of 2026-11-20): a docs
// page rendered from the SDK's committed API report,
// packages/sdk/etc/api-report.d.ts, one section per module, so the page is
// never older than the report the `api-report` gate keeps current.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { API_REFERENCE, apiReferenceEntry } from "../src/api-reference.mjs";

const site = join(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(site, "dist");
const report = readFileSync(
  join(site, "..", "packages/sdk/etc/api-report.d.ts"),
  "utf8",
);

test("each module of the report is a section with its declarations in a TypeScript block", () => {
  const page = apiReferenceEntry(
    "// file: account.d.ts\nexport interface A {}\n// file: db.d.ts\nexport declare const b: 1;\n",
  );
  assert.ok(page.title.length > 0);
  assert.ok(page.description.length > 0);
  assert.match(
    page.body,
    /## account\n\n```ts\nexport interface A \{\}\n```\n\n## db\n\n```ts\nexport declare const b: 1;\n```/,
  );
  assert.ok(!page.body.includes("// file:"));
});

test("the page links the API's main calls on the site", () => {
  assert.ok(apiReferenceEntry(report).body.includes("](/docs/api/)"));
});

test("a report without a module marker is refused", () => {
  assert.throws(() => apiReferenceEntry("export interface A {}\n"), /module/);
});

test("the built page holds every module of the committed report", () => {
  const file = join(dist, API_REFERENCE.slug, "index.html");
  assert.ok(existsSync(file), `${API_REFERENCE.slug} is not built`);
  const html = readFileSync(file, "utf8");
  for (const [, module] of report.matchAll(/^\/\/ file: (.*)\.d\.ts$/gm))
    assert.ok(html.includes(`id="${module}"`), module);
});

test("the API page links the reference on the site, not the report on GitHub", () => {
  const html = readFileSync(join(dist, "docs/api/index.html"), "utf8");
  assert.ok(html.includes(`href="/${API_REFERENCE.slug}/"`));
  assert.ok(!html.includes("etc/api-report.d.ts"));
});

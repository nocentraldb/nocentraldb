// The README of the npm package @nocentraldb/sdk (NOC-361), which npm shows
// on the package's page: it names the package and says that in 0.1 it runs
// in the browser, links the browser guide and the API reference on the
// website, and its examples type-check against the SDK (doc-snippets.mjs).
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { checkPage } from "./doc-snippets.mjs";

const PATH = "packages/sdk/README.md";

test("the npm package has a README that says what it is and where 0.1 runs", () => {
  assert.ok(existsSync(PATH), `${PATH} is missing`);
  const text = readFileSync(PATH, "utf8");
  assert.match(text, /^# @nocentraldb\/sdk$/m);
  assert.match(text, /0\.1 is a pre-release/);
  assert.match(text, /Chromium, Firefox and Safari/);
  for (const link of [
    "https://nocentraldb.io/docs/guides/browser/",
    "https://nocentraldb.io/docs/api/reference/",
  ])
    assert.ok(text.includes(`](${link})`), link);
});

test("the npm package's README examples type-check against the SDK", () => {
  const { ok, output, count } = checkPage(PATH);
  assert.ok(ok, output);
  assert.ok(count >= 2, `only ${count} snippets`);
});

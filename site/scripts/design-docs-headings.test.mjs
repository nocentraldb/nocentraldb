// Design proposal D2 (NOC-360): in the docs a section heading is clearly
// smaller than the page title. Starlight's h2 is 83 % of its h1 on a desktop,
// and in the brand's heavy display face the two read as equals.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const dist = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");

test("the docs set h2 and h3 below Starlight's sizes", () => {
  const html = readFileSync(join(dist, "docs/index.html"), "utf8");
  assert.match(html, /--sl-text-h2:\s*1\.75rem/);
  assert.match(html, /--sl-text-h3:\s*1\.375rem/);
});

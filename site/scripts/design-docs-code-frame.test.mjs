// Design proposal D1 (NOC-360): a shell command in the docs sits in a plain
// code frame, without the empty terminal title bar that took a third of its height.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const dist = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");

test("the docs show shell commands without a terminal frame", () => {
  const html = readFileSync(join(dist, "docs/index.html"), "utf8");
  assert.ok(html.includes("npm install"), "the page has a shell command");
  assert.ok(!html.includes("is-terminal"), "a terminal frame is left");
});

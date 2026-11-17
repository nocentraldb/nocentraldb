// Design proposal D4 (NOC-360): a phone's first screen shows no drawing. At
// 390 px the hero's wide drawing is 340 px across and its labels cannot be
// read; the scenario right below shows the same first step in its tall,
// readable layout.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const dist = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");

test("below 640px the hero's drawing is not shown", () => {
  const html = readFileSync(join(dist, "index.html"), "utf8");
  assert.match(
    html,
    /@media\s*\(max-width:\s*639px\)\s*\{\s*\.hero \.hero__media\s*\{\s*display:\s*none\s*\}\s*\}/,
  );
});

// Design proposal D3 (NOC-360): on a desktop the code and the demo sections
// are as tall as their content, not one full screen each, which left a third
// of each screen empty at 1440x900. It departs from the kit's rhythm of one
// section per screen for these two sections only.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const dist = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");

test("on a desktop the code and demo sections take their content's height", () => {
  const html = readFileSync(join(dist, "index.html"), "utf8");
  assert.match(
    html,
    /html main>section:is\(#code,#demo\)\{height:auto;min-height:0/,
  );
});

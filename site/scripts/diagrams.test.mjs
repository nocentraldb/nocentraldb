// The RFC diagrams on the docs page "How NoCentralDB works" (NOC-360, the
// owner's point 19 of 2026-11-19): site copies in the brand's dark palette,
// made from the RFCs' own files by scripts/diagrams.mjs, which changes only
// their style; the RFCs' files stay as they are.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { DIAGRAMS, siteCopy } from "./diagrams.mjs";

const site = join(dirname(fileURLToPath(import.meta.url)), "..");
const rfc = join(site, "..", "docs", "rfc");

test("each site copy is its RFC diagram in the brand's palette, and current", () => {
  assert.ok(DIAGRAMS.length >= 3);
  for (const name of DIAGRAMS) {
    const original = readFileSync(join(rfc, name), "utf8");
    const copy = readFileSync(join(site, "public", "diagrams", name), "utf8");
    assert.equal(
      copy,
      siteCopy(original),
      `${name} is out of date: run node site/scripts/diagrams.mjs`,
    );
    for (const light of ["#ffffff", "#f6f8fa", "#0969da", "#ddf4ff", "#4493f8"])
      assert.ok(!copy.includes(light), `${name} keeps ${light}`);
    assert.ok(
      !copy.includes("prefers-color-scheme"),
      `${name} keeps a light theme`,
    );
    assert.ok(
      copy.includes("#111110") && copy.includes("#ecebe4"),
      `${name} is not in the brand's colours`,
    );
    // the drawing itself is the RFC's: the same labels
    const labels = (svg) =>
      [...svg.matchAll(/<text[^>]*>([^<]*)</g)].map((m) => m[1]);
    assert.deepEqual(labels(copy), labels(original), `${name} lost a label`);
  }
});

// Each RFC from 0002 on keeps its opening short and lists how it relates to
// the other RFCs in a section at the end, before its Decisions and History
// (Carsten, 2026-11-20, NOC-362).
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { test } from "node:test";
import { read } from "./lib/rfc-text.mjs";

const rfcs = readdirSync("docs/rfc")
  .filter((f) => /^\d{4}-.*\.md$/.test(f) && !f.startsWith("0001"))
  .map((f) => `docs/rfc/${f}`);

/** The level-2 headings, outside code blocks. */
const h2 = (markdown) =>
  markdown
    .replace(/^```[\s\S]*?^```$/gm, "")
    .split("\n")
    .filter((l) => l.startsWith("## "))
    .map((l) => l.slice(3));

for (const path of rfcs) {
  test(`${path} ends with how it relates to the others`, () => {
    const found = h2(read(path));
    const at = found.indexOf("How this RFC relates to the others");
    assert.ok(at >= 0, "no section How this RFC relates to the others");
    assert.ok(
      ["Decisions", "History"].includes(found[at + 1]),
      `it is followed by ${found[at + 1]}`,
    );
  });

  test(`${path} opens with no paragraph longer than two sentences on what it builds on`, () => {
    const intro = read(path).split(/^## /m)[0];
    const builds = intro
      .split("\n")
      .filter((l) => /^It (also )?builds on\b/.test(l));
    for (const line of builds) {
      const sentences = line.split(/(?<=[.!?])\s+(?=[A-Z[])/).length;
      assert.ok(sentences <= 2, `${sentences} sentences: ${line.slice(0, 80)}`);
    }
  });
}

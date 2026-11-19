// The RFCs use the glossary's terms (docs/glossary.md, NOC-362). Each pattern
// below is a word from the glossary's "Avoid" column in the sense the
// glossary avoids it. An RFC uses the glossary's term instead: "another
// member" for a peer, "sign-off" for approval of an erasure, "converge" for
// copies that agree, "object" for a record of the model, "core" for the
// engine, and "`write`" for a write transaction of the app (a SQLite or
// DynamoDB write transaction keeps its name). Code blocks and
// the History lines are left out; RFC 0005 explains its own pair of terms,
// schema and model, where it defines them.
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { test } from "node:test";
import { read } from "./lib/rfc-text.mjs";

const rfcs = readdirSync("docs/rfc")
  .filter((file) => /^\d{4}-.*\.md$/.test(file))
  .map((file) => `docs/rfc/${file}`);

/** The prose of an RFC: no fenced code, no History section. */
const prose = (path) =>
  read(path)
    .replace(/^```[\s\S]*?^```$/gm, "")
    .replace(/^## History[\s\S]*?(?=^## |(?![\s\S]))/m, "");

const AVOIDED = [
  [/\bpeers?\b/i, "another member, directly"],
  [/\bis a cache\b/i, "the view is computed from the log"],
  [/\bErasure approvers\b|\bapproves none\b|\bperson's approval\b/, "sign-off"],
  [/\bCopies agree\b|\bThe relay agrees\b/, "converge"],
  [/\ba phone, a browser profile\b/, "a database in a browser"],
  [/\brecords per task\b|\baccount's own records\b/, "object"],
  [/\bthe engine turns\b/, "the core"],
  [/(?<!small |One )\bwrite transaction\b/, "one `write`"],
  [/\bA transaction that (writes|adds|removes|sets)\b/, "a `write`"],
];

for (const path of rfcs) {
  test(`${path} uses the glossary's terms`, () => {
    const text = prose(path);
    for (const [pattern, instead] of AVOIDED) {
      const hit = text.match(pattern);
      assert.ok(!hit, `${path} says "${hit?.[0]}"; the glossary says: ${instead}`);
    }
  });
}

test("RFC 0005 explains its schema and model against the glossary", () => {
  assert.match(
    read("docs/rfc/0005-schema-and-model-lock.md"),
    /The \[glossary\]\(\.\.\/glossary\.md\) calls the TypeScript module the app's model in code/,
  );
});

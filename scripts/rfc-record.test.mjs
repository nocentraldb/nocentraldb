// The record of an RFC's later changes (scripts/lib/rfc-record.mjs, NOC-362):
// History starts with the publication and adds one dated line per later
// change; a Decisions section appears only after publication, directly
// before History, and each entry is dated, says what changed and links its
// decision file, or names its ticket where the decision has no entry in the
// log. An entry names nobody who decided: the decision log records that
// (Carsten, 2026-11-23, NOC-454 B). These tests feed the helper small
// RFCs, so a helper that lets a wrong record through fails here.
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { assertRecord, historyLines } from "./lib/rfc-record.mjs";
import { read, root } from "./lib/rfc-text.mjs";

const PUBLISHED = "2026-11-10 · First published (NOC-165)";
const FILE =
  "2026-11-10-NOC-100-rfcs-are-rewritten-as-complete-texts-without-amendments.md";

const rfc = ({ decisions, history }) =>
  [
    "# RFC 9999: A test",
    "",
    "## The design",
    "",
    "Text.",
    "",
    ...(decisions === undefined ? [] : ["## Decisions", "", ...decisions, ""]),
    "## History",
    "",
    ...history,
    "",
    "## How this RFC is tested",
    "",
    "Tests.",
    "",
  ].join("\n");

test("a record of the publication alone holds", () => {
  assertRecord(rfc({ history: [`- ${PUBLISHED}`] }), PUBLISHED);
});

test("later History lines and a Decisions entry hold", () => {
  assertRecord(
    rfc({
      decisions: [
        `- **2026-11-12 · Personal values are sealed under a subkey.** ([decision](../../factory/decisions/${FILE}))`,
        "- **2026-11-13 · A ticket's decision.** (NOC-298)",
      ],
      history: [
        `- ${PUBLISHED}`,
        "- 2026-11-12 · Personal values are sealed under a subkey (NOC-244, #248)",
        "- 2026-11-13 · A member id prints as hex (NOC-298, NOC-299, #310)",
        "- 2026-11-19 · Terms follow the glossary (NOC-362)",
      ],
    }),
    PUBLISHED,
  );
});

test("History must start with the publication", () => {
  assert.throws(() =>
    assertRecord(
      rfc({ history: ["- 2026-11-12 · Something (NOC-1, #2)"] }),
      PUBLISHED,
    ),
  );
});

test("a later History line needs a date, the change and its ticket", () => {
  for (const line of [
    "- Something changed (NOC-1, #2)",
    "- 2026-11-12 · Something changed",
    "- 2026-11-12 · Something changed (#2)",
  ]) {
    assert.throws(
      () => assertRecord(rfc({ history: [`- ${PUBLISHED}`, line] }), PUBLISHED),
      undefined,
      line,
    );
  }
});

test("History runs in date order", () => {
  assert.throws(() =>
    assertRecord(
      rfc({
        history: [
          `- ${PUBLISHED}`,
          "- 2026-11-13 · Later (NOC-2, #3)",
          "- 2026-11-12 · Earlier (NOC-1, #2)",
        ],
      }),
      PUBLISHED,
    ),
  );
});

test("Decisions needs a later History line", () => {
  assert.throws(() =>
    assertRecord(
      rfc({
        decisions: ["- **2026-11-12 · A decision.** (NOC-1)"],
        history: [`- ${PUBLISHED}`],
      }),
      PUBLISHED,
    ),
  );
});

test("Decisions stands directly before History", () => {
  const text = rfc({
    decisions: ["- **2026-11-12 · A decision.** (NOC-1)"],
    history: [`- ${PUBLISHED}`, "- 2026-11-12 · A decision (NOC-1, #2)"],
  }).replace("## History", "## Something between\n\nText.\n\n## History");
  assert.throws(() => assertRecord(text, PUBLISHED));
});

test("an entry is dated and names its source, and names nobody who decided", () => {
  for (const entry of [
    "- A decision without a date (NOC-1).",
    "- **2026-11-12 · A decision.** No source.",
    "- **2026-11-12 · A decision.** ([decision](../../factory/decisions/2026-11-12-NOC-1-no-such-file.md))",
    "- **2026-11-12 · A decision.** (Carsten, 2026-11-12, NOC-1)",
    "- **2026-11-12 · A decision.** (pm, 2026-11-12, NOC-1)",
    `- **2026-11-12 · A decision.** (pm, 2026-11-22, NOC-379, recording the coding agent's call on NOC-57 as shipped in #215; [decision](../../factory/decisions/${FILE}))`,
  ]) {
    assert.throws(
      () =>
        assertRecord(
          rfc({
            decisions: [entry],
            history: [
              `- ${PUBLISHED}`,
              "- 2026-11-12 · A decision (NOC-1, #2)",
            ],
          }),
          PUBLISHED,
        ),
      undefined,
      entry,
    );
  }
});

// Every RFC's own record holds, RFC 0012 included, which has no test file of
// its own that checks it.
for (const file of readdirSync(join(root, "docs/rfc")).filter((f) =>
  /^\d{4}-.*\.md$/.test(f),
)) {
  test(`docs/rfc/${file} keeps its record in the form`, () => {
    const text = read(`docs/rfc/${file}`);
    const [first] = historyLines(text);
    assert.match(first, /^- \d{4}-\d{2}-\d{2} · First published\b/);
    assertRecord(text, first.slice(2));
  });
}

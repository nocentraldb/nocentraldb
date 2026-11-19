// The record of an RFC's later changes (scripts/lib/rfc-record.mjs, NOC-362):
// History starts with the publication and adds one dated line per later
// change; a Decisions section appears only after publication, directly
// before History, and each entry is dated, names who decided it and when,
// and names its decision file or ticket. These tests feed the helper small
// RFCs, so a helper that lets a wrong record through fails here.
import assert from "node:assert/strict";
import { test } from "node:test";
import { assertRecord } from "./lib/rfc-record.mjs";

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
        `- **2026-11-12 · Personal values are sealed under a subkey.** The person key seals a personal value (Carsten, 2026-11-12; [decision](../../factory/decisions/${FILE})).`,
        "- **2026-11-13 · A ticket's decision.** Recorded on the ticket (pm, 2026-11-13, NOC-298).",
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
        decisions: [
          "- **2026-11-12 · A decision.** Text (Carsten, 2026-11-12, NOC-1).",
        ],
        history: [`- ${PUBLISHED}`],
      }),
      PUBLISHED,
    ),
  );
});

test("Decisions stands directly before History", () => {
  const text = rfc({
    decisions: [
      "- **2026-11-12 · A decision.** Text (Carsten, 2026-11-12, NOC-1).",
    ],
    history: [`- ${PUBLISHED}`, "- 2026-11-12 · A decision (NOC-1, #2)"],
  }).replace("## History", "## Something between\n\nText.\n\n## History");
  assert.throws(() => assertRecord(text, PUBLISHED));
});

test("an entry is dated, names who decided and when, and its source", () => {
  for (const entry of [
    "- A decision without a date (Carsten, 2026-11-12, NOC-1).",
    "- **2026-11-12 · A decision.** Nobody decided it (NOC-1).",
    "- **2026-11-12 · A decision.** No source (Carsten, 2026-11-12).",
    "- **2026-11-12 · A decision.** A missing file (Carsten, 2026-11-12; [decision](../../factory/decisions/2026-11-12-NOC-1-no-such-file.md)).",
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

// The record of a decided RFC's later changes, as docs/rfc/README.md ("How a
// decided RFC changes") and the owner's decision of 2026-11-10 ask for it
// (factory/decisions/2026-11-10-NOC-100-rfcs-are-rewritten-as-complete-texts-without-amendments.md):
// a change is written into the text, a decision made after publication is
// kept in short form under "Decisions", and "History" starts with the
// publication line and adds one dated line per later change (NOC-362).
//
// Every RFC's own test calls `assertRecord` in place of a check for exactly
// one History line and no "Decisions" section.
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { headings, root, section, slug } from "./rfc-text.mjs";

/** A later History line: date, what changed, then the ticket(s) and, when known, the pull request. */
export const LATER =
  /^- (\d{4}-\d{2}-\d{2}) · \S.*\((NOC-\d+)(, NOC-\d+)*(, #\d+)?\)$/;

/** A Decisions entry: a bold dated title that says what changed, then its source. */
export const ENTRY = /^- \*\*(\d{4}-\d{2}-\d{2}) · \S.*\*\*/;

/** The non-empty lines of a section, without its sub-headings. */
const lines = (text) => text.split("\n").filter((l) => l.trim());

/** The non-empty lines of the RFC's History. */
export const historyLines = (markdown) => lines(section(markdown, "history"));

/** Asserts that History is the publication line, then dated later lines in order. */
export const assertHistory = (markdown, published) => {
  const [first, ...later] = historyLines(markdown);
  assert.equal(first, `- ${published}`, "History starts with the publication");
  const start = published.slice(0, 10);
  let previous = start;
  for (const line of later) {
    const m = line.match(LATER);
    assert.ok(
      m,
      `a later History line is "- YYYY-MM-DD · what changed (NOC-n, #m)": ${line}`,
    );
    assert.ok(m[1] >= previous, `History is out of date order at: ${line}`);
    previous = m[1];
  }
};

/** The top-level entries of the Decisions section, each with its continuation lines. */
export const decisionEntries = (markdown) => {
  const entries = [];
  for (const line of section(markdown, "decisions").split("\n")) {
    if (line.startsWith("- ")) entries.push(line);
    else if (line.trim() && entries.length)
      entries[entries.length - 1] += `\n${line}`;
  }
  return entries;
};

/**
 * Asserts the Decisions section's place and form. It exists only once History
 * has a line after the publication; it stands directly before History; each
 * entry is dated and names its source: a decision file of the log, which must
 * exist, or the ticket. An entry names nobody who decided: the decision log
 * records that (Carsten, 2026-11-23, NOC-454 B).
 */
export const assertDecisions = (markdown) => {
  const h2 = headings(markdown)
    .filter((h) => !h.startsWith("#"))
    .filter((h) => markdown.includes(`\n## ${h}\n`))
    .map(slug);
  if (!h2.includes("decisions")) return;
  assert.ok(
    historyLines(markdown).length > 1,
    "a Decisions section needs a History line after the publication",
  );
  assert.equal(
    h2[h2.indexOf("decisions") + 1],
    "history",
    "Decisions stands directly before History",
  );
  const entries = decisionEntries(markdown);
  assert.ok(entries.length > 0, "Decisions holds no entry");
  for (const entry of entries) {
    const title = entry.split("\n")[0];
    assert.match(
      title,
      ENTRY,
      `an entry starts "- **YYYY-MM-DD · …**": ${title}`,
    );
    assert.doesNotMatch(
      entry,
      /\b(Carsten|pm)\b/,
      `an entry names who decided it; the decision log records that: ${title}`,
    );
    const files = [
      ...entry.matchAll(/\]\(\.\.\/\.\.\/factory\/decisions\/([^)#\s]+)\)/g),
    ].map((m) => m[1]);
    for (const file of files) {
      assert.ok(
        existsSync(join(root, "factory/decisions", file)),
        `an entry links a decision file that does not exist: ${file}`,
      );
    }
    assert.ok(
      files.length > 0 || /\bNOC-\d+\b/.test(entry),
      `an entry links its decision file or names its ticket: ${title}`,
    );
  }
};

/** Asserts the whole record: History, and Decisions where there is one. */
export const assertRecord = (markdown, published) => {
  assertHistory(markdown, published);
  assertDecisions(markdown);
};

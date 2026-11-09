// Asserts the proposal of NOC-34: the developer page "Browser apps and
// durability" says nothing NOC-33 did not find. Every row of its tables names,
// in its Source cell, the rows of the findings table in
// docs/browser-storage.md it rests on, with the same kind of source
// (measured, documented or unknown), so a statement cannot claim more than
// its row. Runs in `pnpm gates` without a browser.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { tableRows } from "./browser-storage/report.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const PAGE = "docs/browser-durability.md";
const FINDINGS = "docs/browser-storage.md";

const read = (path) => readFileSync(join(root, path), "utf8");

/** The text of the level-2 section titled `title`, without its heading. */
const section = (markdown, title) => {
  const part = markdown
    .split(/^(?=## )/m)
    .find((chunk) => chunk.split("\n")[0].trim() === `## ${title}`);
  return part?.split("\n").slice(1).join("\n");
};

/** The kind of a findings row's source: measured, documented or unknown. */
const kind = (source) => source.match(/^(measured|documented|unknown)/)?.[1];

/**
 * The references in a Source cell: "kind: Browser · Behaviour", several
 * separated by "; ". Anything else in the cell is not a reference.
 */
const references = (cell) =>
  cell
    .split(/;\s*/)
    .map((part) => part.trim())
    .filter(Boolean)
    .map((part) => {
      const match = part.match(/^(measured|documented|unknown): (.+?) · (.+)$/);
      return match
        ? { kind: match[1], browser: match[2], behaviour: match[3] }
        : { invalid: part };
    });

/** Every traced table on the page: those with a Source column. */
const tracedTables = () => {
  const page = read(PAGE);
  const titles = [...page.matchAll(/^## (.+)$/gm)].map((match) => match[1]);
  return titles
    .map((title) => {
      const body = section(page, title);
      if (!/^\| .* \| Source \|$/m.test(body ?? "")) return undefined;
      return { title, rows: tableRows(page, title) };
    })
    .filter(Boolean);
};

test("the page Browser apps and durability exists", () => {
  assert.ok(existsSync(join(root, PAGE)), `${PAGE} is missing`);
  assert.match(read(PAGE), /^# Browser apps and durability$/m);
});

test("the page has the sections the ticket asks for", () => {
  const page = read(PAGE);
  for (const title of [
    "Each browser in plain words",
    "What is lost, and when",
    "How to reduce the risk",
    "Private windows",
    "What the SDK tells you",
    "Text for your users",
  ]) {
    assert.ok(section(page, title) !== undefined, `${PAGE}: no "## ${title}"`);
  }
});

test("the page's tables trace every row to the findings of NOC-33", () => {
  const tables = tracedTables();
  for (const title of [
    "Each browser in plain words",
    "What is lost, and when",
    "How to reduce the risk",
  ]) {
    assert.ok(
      tables.some((table) => table.title === title),
      `${PAGE}: "${title}" has no table with a Source column`,
    );
  }
  const findings = tableRows(read(FINDINGS), "Findings");
  for (const { title, rows } of tables) {
    assert.ok(rows.length > 0, `${PAGE}: "${title}" has no rows`);
    for (const row of rows) {
      const refs = references(row.source);
      const label = `${PAGE}, "${title}": ${Object.values(row)[0]}`;
      assert.ok(refs.length > 0, `${label}: the Source cell names no row`);
      for (const ref of refs) {
        assert.ok(!ref.invalid, `${label}: "${ref.invalid}" is no reference`);
        const found = findings.find(
          (finding) =>
            finding.browser === ref.browser &&
            finding.behaviour === ref.behaviour,
        );
        assert.ok(
          found,
          `${label}: ${FINDINGS} has no row "${ref.browser} · ${ref.behaviour}"`,
        );
        assert.equal(
          kind(found.source),
          ref.kind,
          `${label}: "${ref.browser} · ${ref.behaviour}" is ${kind(found.source)}`,
        );
      }
    }
  }
});

test("a row that rests only on unknown findings says it is unknown", () => {
  for (const { title, rows } of tracedTables()) {
    for (const row of rows) {
      const refs = references(row.source);
      if (refs.length === 0 || refs.some((ref) => ref.kind !== "unknown")) {
        continue;
      }
      const statement = Object.entries(row)
        .filter(([column]) => column !== "source")
        .map(([, value]) => value)
        .join(" ");
      assert.match(
        statement,
        /\bunknown\b/i,
        `${PAGE}, "${title}": ${Object.values(row)[0]} rests only on unknown rows`,
      );
    }
  }
});

// Asserts that a decided RFC never calls itself a draft (NOC-35). An RFC is
// decided once its status paragraph says so: it starts with "Decided," and
// the date, and names nobody who decided, since the decision log records
// that (Carsten, 2026-11-23, NOC-454 B). The RFC's own status line is the
// record; the decision log holds the single decisions, each under a title
// that states it (NOC-378), so no log heading marks a whole RFC as decided. Such an RFC's header line carries no draft number, and no
// sentence in its status still waits for its own pull request to be merged.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

/** The owner, as the row "Owner (the source)" of factory/CONFIG.md names them. */
const owner = () => {
  const row = read("factory/CONFIG.md")
    .split("\n")
    .find((line) => line.startsWith("| Owner (the source)"));
  assert.ok(row, "factory/CONFIG.md has no row for the owner");
  return row.split("|")[2].trim();
};

/** The numbers of the RFCs whose status paragraph says they are decided. */
const decidedRfcs = () =>
  readdirSync(join(root, "docs/rfc"))
    .filter((f) => /^\d{4}-.*\.md$/.test(f))
    .filter((f) => /^\*\*Status\.\*\* Decided\b/m.test(read(`docs/rfc/${f}`)))
    .map((f) => f.slice(0, 4))
    .sort();

const rfcFile = (number) => {
  const file = readdirSync(join(root, "docs/rfc")).find(
    (f) => f.startsWith(`${number}-`) && f.endsWith(".md"),
  );
  assert.ok(
    file,
    `the decision log names RFC ${number}, but docs/rfc has no such file`,
  );
  return `docs/rfc/${file}`;
};

/** The paragraph that starts with "**Status.**". */
const status = (markdown) =>
  markdown.split("\n").find((line) => line.startsWith("**Status.**"));

test("RFC 0001 to RFC 0009 say they are decided", () => {
  // RFC 0001 counts as decided since the owner confirmed it with NOC-165.
  assert.deepEqual(decidedRfcs().slice(0, 9), [
    "0001",
    "0002",
    "0003",
    "0004",
    "0005",
    "0006",
    "0007",
    "0008",
    "0009",
  ]);
});

test("a decided RFC's header line carries no draft number", () => {
  for (const number of decidedRfcs()) {
    const path = rfcFile(number);
    const header = read(path).split("\n")[2];
    assert.ok(!/draft/i.test(header), `${path}: header says draft: ${header}`);
  }
});

test('a decided RFC\'s status reads "Decided, YYYY-MM-DD." and names nobody', () => {
  const name = owner();
  for (const number of decidedRfcs()) {
    const path = rfcFile(number);
    const line = status(read(path));
    assert.ok(line, `${path} has no status paragraph`);
    assert.match(
      line,
      /^\*\*Status\.\*\* Decided, \d{4}-\d{2}-\d{2}\.( |$)/,
      `${path}: ${line.slice(0, 80)}`,
    );
    assert.doesNotMatch(
      line,
      new RegExp(`\\b(${name}|pm)\\b|\\bthe owner\\b`),
      `${path}: the status names who decided; the decision log records that`,
    );
  }
});

test("a decided RFC's status no longer waits for its own pull request", () => {
  for (const number of decidedRfcs()) {
    const path = rfcFile(number);
    const line = status(read(path)) ?? "";
    assert.ok(
      !/(final|built) (when|until) this (pull request|RFC) is merged/i.test(
        line,
      ),
      `${path}: the status still waits for its merge`,
    );
  }
});

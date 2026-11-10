// Asserts that a merged RFC never calls itself a draft (NOC-35). An RFC counts
// as merged once the decision log records the owner's decision on it as a
// whole: a file in factory/decisions/ whose heading reads "RFC <number>: ..."
// or "The owner decides RFC <number>, ..." (NOC-64's form), and whose "Decided by:" names the owner from factory/CONFIG.md. The gate
// cannot ask git which files are on main (CI checks out one commit), so the
// decision log, which the gate already holds to its format, is the record.
// Such an RFC's header line carries no draft number, its status paragraph
// starts with "Decided." or, once rewritten as a fresh text (NOC-165), with
// "Decided by the owner", and names the owner with a date, and no sentence in it
// still waits for its own pull request to be merged.
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

/** The numbers of the RFCs the owner decided as a whole, from the decision log. */
const decidedRfcs = () => {
  const name = owner();
  const numbers = new Set();
  for (const file of readdirSync(join(root, "factory/decisions"))) {
    if (!file.endsWith(".md")) continue;
    const text = read(`factory/decisions/${file}`);
    const rfc = text.match(
      /^# [^\n]*· (?:The owner decides )?RFC (\d{4})[:,] /,
    )?.[1];
    if (rfc && new RegExp(`^- Decided by: ${name}\\b`, "m").test(text)) {
      numbers.add(rfc);
    }
  }
  return [...numbers].sort();
};

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

test("the decision log names the RFCs the owner decided", () => {
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

test("a decided RFC's status says Decided, with the owner and a date", () => {
  const name = owner();
  for (const number of decidedRfcs()) {
    const path = rfcFile(number);
    const line = status(read(path));
    assert.ok(line, `${path} has no status paragraph`);
    assert.match(
      line,
      /^\*\*Status\.\*\* Decided(\. | by the owner )/,
      `${path}: ${line.slice(0, 80)}`,
    );
    assert.match(
      line,
      new RegExp(`\\(${name}, \\d{4}-\\d{2}-\\d{2}\\)`),
      `${path}: the status names no decision by ${name} with a date`,
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

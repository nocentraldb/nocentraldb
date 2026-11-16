// Every citation of an RFC by id names an id the RFC still defines
// (NOC-170). The fresh texts of NOC-165 removed the decision ids, such as
// "RFC 0009, S19" or "RFC 0003, decision 17"; code, docs and change
// fragments now cite the section instead. The ids that remain are the
// first column of an RFC's tables, such as RFC 0004's properties C1 to C7,
// and may still be cited. The decision log (factory/decisions/) records
// what was decided when, under the names of the time, and is left as it is.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const RFC_DIR = `${ROOT}docs/rfc`;

/** The ids an RFC text defines: the first cell of a table row. */
const definedIds = (text) =>
  new Set(
    [...text.matchAll(/^\|\s*([A-Z]{1,2}(?:-[A-Z])?\d+)\s*\|/gm)].map(
      (match) => match[1],
    ),
  );

/**
 * A citation of one of this project's RFCs by id: "RFC 0009, S19",
 * "RFC 0003, decision 17". A list of RFC numbers ("RFC 0001, 0002") or a
 * standard's section ("RFC 8032, 7") is no such citation.
 */
const CITATION = /RFC (00\d\d),? (?:decisions? )?([A-Z]{0,2}\d{1,3})\b/g;

/** The citations in `text` whose id `rfcs` (number → defined ids) lacks. */
const staleCitations = (text, rfcs) => {
  const stale = [];
  text.split("\n").forEach((line, index) => {
    for (const [cited, rfc, id] of line.matchAll(CITATION)) {
      if (!rfcs.get(rfc)?.has(id)) stale.push({ line: index + 1, cited });
    }
  });
  return stale;
};

/** Each RFC's number and the ids it defines. */
const rfcIds = () =>
  new Map(
    readdirSync(RFC_DIR)
      .filter((name) => /^\d{4}-.*\.md$/.test(name))
      .map((name) => [
        name.slice(0, 4),
        definedIds(readFileSync(`${RFC_DIR}/${name}`, "utf8")),
      ]),
  );

/** The tracked text files a citation may stand in. */
const trackedTexts = () =>
  execFileSync("git", ["ls-files", "-z"], { cwd: ROOT, encoding: "utf8" })
    .split("\0")
    .filter(
      (file) =>
        file !== "" &&
        !file.startsWith("factory/decisions/") &&
        // This file's own examples.
        file !== "scripts/rfc-citations.test.mjs" &&
        /\.(md|rs|toml|ts|tsx|js|mjs|cjs|json|ya?ml|sh|astro|html|txt)$/.test(
          file,
        ),
    );

test("the matcher finds removed ids, and passes ids that remain, lists of RFCs and standards", () => {
  const rfcs = new Map([["0004", new Set(["C1", "C3"])]]);
  const text = [
    "keeps both in memory (RFC 0009, S19).",
    "its key (RFC 0003, decision 17).",
    "converges (RFC 0004, C1 and C4).",
    "RFC 0004's property C3, RFC 0004, C3.",
    "RFC 0001, 0002, 0003 and the FAQ.",
    "the vectors of RFC 8032, 7.1.",
  ].join("\n");
  assert.deepEqual(staleCitations(text, rfcs), [
    { line: 1, cited: "RFC 0009, S19" },
    { line: 2, cited: "RFC 0003, decision 17" },
  ]);
});

test("RFC 0004 defines its properties C1 to C7, and RFC 0008 its M-C properties", () => {
  const rfcs = rfcIds();
  for (const id of ["C1", "C2", "C3", "C4", "C5", "C6", "C7"]) {
    assert.ok(rfcs.get("0004").has(id), `RFC 0004 defines ${id}`);
  }
  assert.ok(rfcs.get("0008").has("M-C1"), "RFC 0008 defines M-C1");
});

test("no code, doc or change fragment cites an id its RFC no longer defines", () => {
  const rfcs = rfcIds();
  const stale = trackedTexts().flatMap((file) =>
    staleCitations(readFileSync(`${ROOT}${file}`, "utf8"), rfcs).map(
      ({ line, cited }) => `${file}:${line}: ${cited}`,
    ),
  );
  assert.deepEqual(
    stale,
    [],
    "cite the RFC's section by name instead (NOC-170)",
  );
});

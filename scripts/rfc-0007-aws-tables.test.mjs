// Asserts that RFC 0007's AWS backend carries the five points the owner
// decided with the tick on #275 (NOC-270), each marked with his name, the
// date and the pull request: the tables `uploads` and `tokens`, ids in
// lowercase hex in S3 keys, the epoch item's key `epoch`, the index
// `by_member` on `members`, and the `ttl` attribute on uploads and tokens;
// and that every place that lists the backend's tables lists the two new
// ones. Whether the wording is right is the owner's review of the PR.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0007-relay.md";
const MARK = "(Carsten, 2026-11-12, #275)";
const read = () => readFileSync(join(root, RFC), "utf8");

/** The text under the heading `### title`, up to the next `##` heading. */
const subsection = (title) => {
  const markdown = read();
  const start = markdown.indexOf(`\n### ${title}\n`);
  assert.ok(start >= 0, `${RFC} has no section "${title}"`);
  const rest = markdown.slice(start + 1);
  const end = rest.slice(1).search(/\n##/);
  return end < 0 ? rest : rest.slice(0, end + 1);
};

/** The row of the tables' table whose first cell names `table`. */
const row = (table) => {
  const line = subsection("Tables and keys")
    .split("\n")
    .find((l) => l.startsWith(`| \`${table}\` |`));
  assert.ok(line, `"Tables and keys" has no row for \`${table}\``);
  return line;
};

test("uploads and download tokens have tables of their own, keyed as decided", () => {
  const uploads = row("uploads");
  assert.match(uploads, /^\| `uploads` \| upload id \| — \|/);
  assert.ok(uploads.includes(MARK), "the uploads row carries the owner's mark");
  const tokens = row("tokens");
  assert.match(tokens, /^\| `tokens` \| token \| — \|/);
  assert.ok(tokens.includes(MARK), "the tokens row carries the owner's mark");
});

test("uploads and tokens carry a ttl attribute", () => {
  for (const table of ["uploads", "tokens"]) {
    assert.match(
      row(table),
      /`ttl`/,
      `the ${table} row names the ttl attribute`,
    );
  }
});

test("the epoch item's key in spaces is the five bytes epoch", () => {
  const spaces = row("spaces");
  assert.match(spaces, /key is the 5 bytes `epoch`/);
  assert.ok(spaces.includes(MARK));
});

test("the index on members is by_member, sorted by space id, with current", () => {
  const members = row("members");
  assert.match(members, /`by_member`/);
  assert.match(members, /sorted by space id/);
  assert.match(members, /`current`/);
  assert.ok(members.includes(MARK));
});

test("S3 keys write their ids in lowercase hex", () => {
  const text = subsection("How large changes go to S3");
  const line = text.split("\n").find((l) => l.includes("lowercase hex"));
  assert.ok(line, "the S3 section says how an id is written in a key");
  assert.ok(line.includes(MARK));
});

test("every list of the backend's tables names uploads and tokens", () => {
  const markdown = read();
  const diagram = markdown.match(/```text\nmember ──WebSocket──▶[\s\S]*?```/);
  assert.ok(diagram, "the AWS diagram is there");
  assert.match(diagram[0], /uploads/);
  assert.match(diagram[0], /tokens/);
  const dynamo = subsection("Services")
    .split("\n")
    .find((l) => l.startsWith("| DynamoDB, on-demand |"));
  assert.ok(dynamo, "the services table has a DynamoDB row");
  assert.match(dynamo, /uploads/);
  assert.match(dynamo, /download tokens/);
});

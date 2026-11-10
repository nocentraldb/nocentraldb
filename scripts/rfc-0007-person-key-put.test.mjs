// Asserts that RFC 0007 describes the backend's person-key put as the relay
// builds it since #130 (NOC-118): the put names the generation each record
// replaces and answers `Stored` or `Moved`, the backend rules hold it as one
// atomic, conditional step, the table says how SQLite, Postgres and DynamoDB
// keep it, and History records the change (NOC-142). Whether the wording is
// right is the owner's review of the PR.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0007-relay.md";
const read = () => readFileSync(join(root, RFC), "utf8");

/** The text under the `## ` heading `title`, up to the next `## ` heading. */
const section = (markdown, title) => {
  const start = markdown.indexOf(`\n## ${title}\n`);
  assert.ok(start >= 0, `${RFC} has no section "${title}"`);
  const rest = markdown.slice(start + 1);
  const end = rest.indexOf("\n## ", 1);
  return end < 0 ? rest : rest.slice(0, end);
};

/** The Rust sketch of the backend interface. */
const sketch = () => {
  const text = section(read(), "The backend interface");
  const found = text.match(/```rust\n([\s\S]*?)```/);
  assert.ok(found, "the backend interface has no Rust sketch");
  return found[1];
};

test("the sketch's put names the generation each record replaces and answers Stored or Moved", () => {
  const rust = sketch();
  assert.match(
    rust,
    /async fn put_person_keys\(\s*&self,\s*space: SpaceId,\s*puts: Vec<PersonKeyPut>,?\s*\)\s*->\s*Result<PutOutcome, StoreError>;/,
  );
  assert.doesNotMatch(
    rust,
    /put_person_keys\([^)]*records: Vec<PersonKeyRecord>/,
  );
  const put = rust.match(/pub struct PersonKeyPut \{([\s\S]*?)\}/);
  assert.ok(put, "the sketch defines no PersonKeyPut");
  assert.match(put[1], /record: PersonKeyRecord/);
  assert.match(put[1], /replaces: Option<u64>/);
  const outcome = rust.match(/pub enum PutOutcome \{([\s\S]*?)\n\}/);
  assert.ok(outcome, "the sketch defines no PutOutcome");
  assert.match(outcome[1], /\bStored\b/);
  assert.match(outcome[1], /\bMoved\b/);
});

test("the backend rules hold the person-key put to the stored generation", () => {
  const text = section(read(), "The backend interface");
  const rule = text
    .split("\n")
    .find((l) =>
      l.startsWith(
        "- **A person-key put is conditional on the stored generation.**",
      ),
    );
  assert.ok(rule, "the rules list has no rule for the person-key put");
  assert.match(rule, /atomic/);
  assert.match(rule, /`Moved`/);
  const row = text
    .split("\n")
    .find((l) => l.startsWith("| Conditional person-key put |"));
  assert.ok(row, "the table does not say how each backend keeps the rule");
  const [sqlite, postgres, dynamodb] = row.split("|").slice(2, 5);
  assert.match(sqlite, /transaction/);
  assert.match(postgres, /WHERE .*generation/);
  assert.match(dynamodb, /condition/i);
  assert.match(dynamodb, /generation/);
});

test("the AWS backend's person_keys table is written on the generation's condition", () => {
  const row = section(read(), "The AWS backend")
    .split("\n")
    .find((l) => l.startsWith("| `person_keys` |"));
  assert.ok(row, "the AWS backend has no person_keys table");
  assert.match(row, /condition/i);
});

test("History names NOC-142 and #130", () => {
  const entry = section(read(), "History")
    .split("\n")
    .find((l) => l.startsWith("- ") && l.includes(" · NOC-142 · "));
  assert.ok(entry, "History has no line for NOC-142");
  assert.match(entry, /^- \*\*\d{4}-\d{2}-\d{2}\*\* · NOC-142 · /);
  assert.match(entry, /#130\b/);
});

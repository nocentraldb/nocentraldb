// Asserts that RFC 0007's "Who may create a space" carries the owner's
// decision on NOC-286 (A1): on a relay that admits only listed accounts,
// an account that is not admitted on its own, but whose member belongs to
// a space an admitted account created, may create exactly one space, its
// account space (RFC 0010), and a second one is refused. The relay cannot
// tell which kind of space it is, since the genesis seals the model id
// (RFC 0008), so the text must not say it does. Whether the wording is
// right is the owner's review of the PR.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0007-relay.md";
const MARK = "(Carsten, 2026-11-12, NOC-286)";

/** The paragraph that starts "**Who may create a space.**". */
const paragraph = () => {
  const text = readFileSync(join(root, RFC), "utf8");
  const found = text
    .split("\n")
    .find((l) => l.startsWith("**Who may create a space.**"));
  assert.ok(found, `${RFC} has no paragraph "Who may create a space"`);
  return found;
};

test("an account that is a member of an admitted space may create exactly one space", () => {
  const text = paragraph();
  assert.ok(text.includes(MARK), "the rule carries the owner's mark");
  assert.match(text, /exactly one space/);
  assert.match(text, /account space/);
  assert.match(text, /second/);
  assert.match(text, /`not_allowed`/);
});

test("the rule does not have the relay read the model id", () => {
  const text = paragraph();
  assert.match(text, /seals the model id/);
  assert.doesNotMatch(text, /relay (reads|recognises|tells)[^.]*model id/);
});

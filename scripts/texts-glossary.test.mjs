// The texts of texts/ follow docs/glossary.md and say one thing in
// one way (NOC-359). Each rule names the glossary row or the texts it keeps
// consistent; the review itself is the owner's, with `pnpm texts`.
import assert from "node:assert/strict";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { readTexts } from "./texts-files.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const TEXTS = readTexts(ROOT).texts;
const text = (id) => {
  const entry = TEXTS.find((t) => t.id === id);
  assert.ok(entry, `texts/ has no ${id}`);
  return entry.text;
};

/** The ids of the texts that match `pattern`. */
const matching = (pattern) =>
  TEXTS.filter((t) => pattern.test(t.text)).map((t) => t.id);

test("no text uses a word the glossary's Avoid column names for its concept", () => {
  const avoided = [
    // query: "filter, search, lookup".
    [/\blookups?\b/i, "query"],
    // sub-agent: "a 'worker' is the browser's Web Worker", so name it.
    [/(?<!Web )\bworker\b/i, "the Web Worker"],
    // copy: "replica, cache, mirror, clone"; view: "cache, read model".
    [/\b(replica|cache|mirror)\b/i, "copy or view"],
    // change: "mutation, operation, transaction"; the bridge's `op` and
    // `ops` are its request's names, not prose.
    [/\b(mutation|transaction)s?\b/i, "change"],
    // invitation: "invite (as a noun)".
    [/\ban invite\b/i, "invitation"],
    // sub-agent: "subagent".
    [/\bsubagents?\b/i, "sub-agent"],
    // lock: "lockfile".
    [/\block ?file\b/i, "the lock"],
    // private window: "incognito (name it only for Chrome)".
    [/\bincognito\b/i, "private window"],
  ];
  for (const [pattern, term] of avoided) {
    assert.deepEqual(matching(pattern), [], `say "${term}" (${pattern})`);
  }
});

test("a text spells lowercase one way", () => {
  assert.deepEqual(matching(/lower-case/), []);
});

test("a text says 'is not a', not 'is no'", () => {
  assert.deepEqual(matching(/\b(is|are) no (?!longer\b)/), []);
});

test("a core.unreadable* text reads after core.viewCorrupt's 'an unreadable'", () => {
  assert.match(text("core.viewCorrupt"), /an unreadable \{what\}$/);
  const pieces = TEXTS.filter((t) => t.id.startsWith("core.unreadable"));
  assert.ok(pieces.length > 10);
  for (const piece of pieces) {
    assert.doesNotMatch(piece.text, /^(a|an|the)\b|'s\b/, piece.id);
  }
});

test("a text that is not one this database knows says so in one way", () => {
  const known = "its version is not one this database knows";
  assert.equal(text("core.pairingReadVersion"), known);
  assert.equal(text("part.invitationVersion"), known);
  assert.equal(text("part.erasureTextVersion"), known);
});

test("words that are no recovery code say so as reading them does", () => {
  const unreadable = text("error.recoveryCodeUnreadable");
  assert.equal(unreadable, `${text("error.notARecoveryCode")}: {why}`);
});

test("a cursor that cannot continue says where to start, in both SDKs", () => {
  const start = "; start from the first page";
  assert.ok(text("ts.cursorOfAnotherDatabase").endsWith(start));
  assert.ok(text("error.cursorOfAnotherQuery").endsWith(start));
});

test("a text that means a member says member, not device", () => {
  // device: "a person's phone, laptop or browser"; an agent is a member too.
  assert.doesNotMatch(text("error.personKeyNotHeld"), /\bdevice\b/);
  assert.doesNotMatch(text("error.noAccountSpace"), /\bdevice\b/);
});

test("an erasure's notices say personal fields, and finalize is no noun", () => {
  // erasure: "making a person's personal fields unreadable".
  assert.deepEqual(matching(/\bpersonal data\b/i), []);
  assert.deepEqual(matching(/\ba finalize\b/), []);
});

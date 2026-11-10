// Asserts that RFC 0003 records the owner's answers that NOC-106 writes into
// its text: where the member key lives, with a key file for a headless agent
// (NOC-95, 1 A), the account key each database draws until the accounts RFC
// (NOC-95, 2 A), and the bytes of an encrypted previous key in a key chain
// entry (NOC-97, A). Each stands where it belongs, without an amendment
// mark, with its decision under "Decisions for the owner" and a dated line
// naming the pull request.
import assert from "node:assert/strict";
import { test } from "node:test";
import { assertLinksResolve, read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0003-keys-and-cryptography.md";

/** The numbered decision `n` under "Decisions for the owner", up to the next one. */
const decision = (n) => {
  const text = section(read(RFC), "decisions-for-the-owner");
  const found = text.split(new RegExp(`^${n}\\. `, "m"))[1];
  assert.ok(found, `${RFC} has no decision ${n}`);
  return found.split(/^\d+\. \*\*/m)[0];
};

/** The paragraphs and table rows of `text` that match `pattern`. */
const passages = (text, pattern) =>
  text.split(/\n(?=\S)/).filter((p) => pattern.test(p));

test("the member key lives in the keystore, or in a key file an agent without one names", () => {
  const row = read(RFC)
    .split("\n")
    .find((l) => l.startsWith("| Member key |"));
  assert.match(row, /platform keystore/);
  assert.match(row, /key file/);
  const member = section(read(RFC), "the-member-key");
  assert.match(member, /headless agent/);
  assert.match(member, /key_file/);
  assert.match(member, /file permissions/);
  assert.match(member, /\[decision 16\]\(#decisions-for-the-owner\)/);
});

test("How the SDK hides keys says plainly what protects a key file", () => {
  const [stores] = passages(
    section(read(RFC), "how-the-sdk-hides-keys"),
    /^- \*\*Stores\*\*/,
  );
  assert.match(stores, /key file/);
  assert.match(stores, /only the file's permissions protect/i);
});

test("until the accounts RFC, each database draws its own account key once", () => {
  const account = section(read(RFC), "the-account-key");
  assert.match(account, /each database draws its own account key once/i);
  assert.match(account, /next to the member key/);
  assert.match(account, /unrelated accounts/);
  assert.match(account, /\[decision 17\]\(#decisions-for-the-owner\)/);
  assert.match(account, /0009-sdk-api\.md#the-rust-sdk/);
});

test("an encrypted previous key is nonce, ciphertext and tag, 72 bytes", () => {
  const subkey = section(read(RFC), "key-chain-subkey");
  assert.match(
    subkey,
    /nonce \(24 bytes\) \|\| ciphertext \(32 bytes\) \|\| tag \(16 bytes\)\s+72 bytes/,
  );
  assert.match(subkey, /XChaCha20-Poly1305, random nonce/);
  assert.match(subkey, /0008-membership\.md#key-chain-entry/);
  assert.match(subkey, /merge rotation/);
  assert.match(subkey, /\[decision 18\]\(#decisions-for-the-owner\)/);
});

test("decisions 16, 17 and 18 record the owner's choice, with the options not taken", () => {
  for (const [n, ticket] of [
    [16, "NOC-95"],
    [17, "NOC-95"],
    [18, "NOC-97"],
  ]) {
    const text = decision(n);
    assert.match(text, /\(a\)/, `decision ${n} has no option (a)`);
    assert.match(text, /\(b\)/, `decision ${n} has no option (b)`);
    assert.match(text, /\*\*Recommendation: \(a\)/, `decision ${n}`);
    assert.match(
      text,
      /Decided \(Carsten, 2026-11-10\): the recommendation, \(a\)\./,
      `decision ${n} records no ruling`,
    );
    assert.ok(text.includes(ticket), `decision ${n} names no ${ticket}`);
  }
});

test("the new text carries no amendment mark", () => {
  const rfc = read(RFC);
  for (const pattern of [/key_file/, /draws its own account key/, /72 bytes/]) {
    for (const p of passages(rfc, pattern)) {
      assert.doesNotMatch(p, /\(Amendment \d/, `marked: ${p.slice(0, 60)}`);
    }
  }
  assert.doesNotMatch(rfc, /^## Amendment 4/m);
});

test("RFC 0003's status and dated list name the decisions of 2026-11-10 and the pull request", () => {
  const rfc = read(RFC);
  const status = rfc.split("\n").find((l) => l.startsWith("**Status.**"));
  assert.match(status, /16 to 18.*Carsten, 2026-11-10/);
  assert.match(rfc, /^- \*\*2026-11-10\*\* · NOC-106 · .*\(#\d+\)\.?$/m);
});

test("every relative link in RFC 0003 resolves, anchors included", () => {
  assertLinksResolve(RFC);
});

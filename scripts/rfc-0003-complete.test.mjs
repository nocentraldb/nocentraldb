// Holds RFC 0003 as one complete text (NOC-100): the rules of its three
// amendments and of NOC-106's answers stand where they belong, and no
// amendment section, mark or changelog is left. Its form as a fresh text, with
// each decision in its section and one History line, is held by
// rfc-0003-fresh.test.mjs (NOC-165). Whether the wording is right is the
// review of the PR.
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { join, normalize } from "node:path";
import { test } from "node:test";
import { assertLinksResolve, read, root, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0003-keys-and-cryptography.md";

/** The Markdown link targets in `text`. */
const targets = (text) =>
  [...text.matchAll(/\]\(([^)\s]+)\)/g)].map((m) => m[1]);
const links = (text, target) => targets(text).some((t) => t.endsWith(target));

/** The text of RFC 0003's section `anchor`, which must exist. */
const part = (anchor) => {
  const text = section(read(RFC), anchor);
  assert.ok(text, `RFC 0003 has no section #${anchor}`);
  return text;
};

/** The first line of `text` that starts with `start`. */
const line = (text, start) => {
  const found = text.split("\n").find((l) => l.startsWith(start));
  assert.ok(found, `no line starts with "${start}"`);
  return found;
};

/** Every Markdown file a reader can follow a link from. */
const documents = () => [
  "README.md",
  "CONTRIBUTING.md",
  ...readdirSync(join(root, "docs"), { recursive: true })
    .filter((file) => file.endsWith(".md"))
    .map((file) => normalize(join("docs", file))),
];

test("RFC 0003 has no amendment section, mark or changelog", () => {
  const rfc = read(RFC);
  assert.doesNotMatch(rfc, /^## Amendment/m, "an amendment section is left");
  assert.doesNotMatch(rfc, /\(Amendment \d+/, "an amendment mark is left");
  assert.doesNotMatch(rfc, /\*\*Changelog\.\*\*/, "the changelog is left");
  assert.doesNotMatch(rfc, /amendment/i, "the text still speaks of amendments");
});

test("no document links an amendment of RFC 0003 or its old decisions heading", () => {
  const found = documents().filter((path) =>
    /0003-keys-and-cryptography\.md#(amendment-|decisions-for-the-owner)/.test(
      read(path),
    ),
  );
  assert.deepEqual(found, []);
  assert.doesNotMatch(read(RFC), /\(#decisions-for-the-owner\)/);
});

test("the sealed payload: content key, nonce, associated data, padding and crates", () => {
  const sealed = part("sealed-payload");
  assert.match(sealed, /"nocentraldb\/content-key"/);
  assert.match(sealed, /suite id \(8 bytes, big-endian\)/);
  assert.match(
    sealed,
    /nonce ‖ ciphertext ‖ tag|nonce \|\| ciphertext \|\| tag/,
  );
  assert.match(sealed, /header map without key 6/);
  assert.match(sealed, /multiple of 256 bytes/);
  assert.match(read(RFC), /chacha20poly1305 =0\.11\.0/);
  assert.match(read(RFC), /hkdf =0\.13\.0/);
  assert.ok(
    links(
      line(
        part("how-a-member-signs-and-encrypts-a-change"),
        "2. **The change next.**",
      ),
      "#sealed-payload",
    ),
  );
});

test("NOC-106's answers stand where they belong", () => {
  const member = part("the-member-key");
  assert.match(member, /key_file/);
  assert.match(member, /file permissions/);
  const account = part("the-account-key");
  assert.match(account, /each database draws its own account key once/i);
  const subkey = part("key-chain-subkey");
  assert.match(
    subkey,
    /nonce \(24 bytes\) \|\| ciphertext \(32 bytes\) \|\| tag \(16 bytes\)\s+72 bytes/,
  );
  assert.match(
    line(part("how-the-sdk-hides-keys"), "- **Stores**"),
    /only the file's permissions protect/i,
  );
});

test("RFC 0008's rules stand in the text: generation ids, exposure, the sign-off, the person-key entry and the wait", () => {
  assert.ok(links(part("the-space-key"), "0008-membership.md#generation-ids"));
  const removing = part("removing-a-member-and-rotating-the-space-key");
  assert.match(removing, /\*\*exposed\*\*/);
  assert.match(removing, /\*\*merge rotation\*\*/);
  assert.match(removing, /cutoff/);
  assert.ok(
    links(part("the-person-key"), "0008-membership.md#person-key-entry"),
  );
  const shredding = part("shredding-a-persons-key");
  assert.match(shredding, /holds an admin member in the space/);
  assert.match(shredding, /once the shred is stable/);
  assert.match(shredding, /alarm/);
  assert.match(part("model-id-subkey"), /"nocentraldb\/model-id"/);
});

test("RFC 0009's names stand in the sketch, and the source of a copy stays on the parent", () => {
  const sdk = part("how-the-sdk-hides-keys");
  assert.match(sdk, /spaces\.create\(\)/);
  assert.match(sdk, /role: "writer"/);
  assert.match(sdk, /nc\.open\(\{ model, lock \}\)/);
  assert.match(part("sub-agents-get-an-ad-hoc-space"), /on the parent only/);
});

test("every relative link in RFC 0003 resolves, anchors included", () => {
  assertLinksResolve(RFC);
});

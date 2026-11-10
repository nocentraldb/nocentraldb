// Holds RFC 0003 as one complete text (NOC-100). The owner wants every RFC to
// state the current design directly (Carsten, 2026-11-10): the rules of its
// three amendments and of NOC-106's answers stand where they belong, no
// amendment section, mark or changelog is left, "Decisions" keeps each
// decision in short form, "Open questions" names what the owner has not
// decided yet, and a dated "History" names the pull request of each change.
// This test replaces rfc-0003-owner-answers.test.mjs and the RFC 0003 parts
// of rfc-0008-alignment and rfc-0009-alignment. Whether the wording is right
// is the review of the PR.
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { join, normalize } from "node:path";
import { test } from "node:test";
import {
  assertLinksResolve,
  read,
  root,
  section,
  slug,
} from "./lib/rfc-text.mjs";

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

/** The decision under "Decisions" whose heading starts with `prefix`, such as "9.". */
const decision = (prefix) => {
  const decisions = part("decisions");
  const heading = decisions
    .split("\n")
    .find((l) => l.startsWith(`### ${prefix}`));
  assert.ok(heading, `no decision "${prefix}"`);
  return section(decisions, slug(heading.slice(4)));
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

test("the status says the RFC is decided and points to Decisions, Open questions and History", () => {
  const status = line(read(RFC), "**Status.**");
  assert.match(status, /^\*\*Status\.\*\* Decided\./);
  assert.match(status, /#27/);
  assert.ok(links(status, "#decisions"));
  assert.ok(links(status, "#open-questions"));
  assert.ok(links(status, "#history"));
});

test("decisions 1 to 18 each say what was chosen, by the owner and when, why, and what was not taken", () => {
  for (let n = 1; n <= 18; n += 1) {
    const text = decision(`${n}.`);
    const date = n <= 15 ? "2026-11-08" : "2026-11-10";
    assert.match(
      text,
      new RegExp(`^Decided \\(Carsten, ${date}\\): `, "m"),
      `decision ${n}: no owner and date`,
    );
    assert.match(text, /^Why: /m, `decision ${n}: no reason`);
    assert.match(text, /^Not taken:$/m, `decision ${n}: no options not taken`);
    assert.match(text, /^- /m, `decision ${n}: lists no option not taken`);
    assert.doesNotMatch(
      text,
      /Recommendation/,
      `decision ${n} is not in short form`,
    );
  }
  for (const [n, ticket] of [
    [16, "NOC-95"],
    [17, "NOC-95"],
    [18, "NOC-97"],
  ]) {
    assert.ok(
      decision(`${n}.`).includes(ticket),
      `decision ${n} names no ${ticket}`,
    );
  }
  assert.match(decision("15."), /model-id subkey/);
  assert.match(decision("15."), /M26/);
});

test("the owner's decision on the bytes of a sealed change is recorded, with each option not taken", () => {
  const text = decision("The bytes of a sealed change");
  assert.match(text, /^Decided \(Carsten, 2026-11-08\): /m);
  assert.match(text, /NOC-30/);
  assert.ok(links(text, "#sealed-payload"));
  const notTaken = text.split(/^Not taken:$/m)[1] ?? "";
  assert.ok(
    (notTaken.match(/^- /gm) ?? []).length >= 10,
    "the ten options not taken are not all listed",
  );
});

test("\"Decisions this RFC follows\" names RFC 0008's and RFC 0009's decisions, each linked", () => {
  const lines = part("decisions-this-rfc-follows")
    .split("\n")
    .filter((l) => l.startsWith("- **"));
  for (const [id, target] of [
    ["M3", "0008-membership.md#the-cutoff"],
    ["M6", "0008-membership.md#generation-ids"],
    ["M9", "0008-membership.md#roles-and-rights"],
    ["M15", "0008-membership.md#two-removals-that-rotate-the-key-at-once"],
    ["M20", "0008-membership.md#effects-that-wait-until-a-change-is-stable"],
    ["M21", "0008-membership.md#effects-that-wait-until-a-change-is-stable"],
    ["M26", "0008-membership.md#genesis"],
    ["conflict 11", "0008-membership.md#person-key-entry"],
    ["S11", "0009-sdk-api.md#spaces"],
    ["point 9", "0009-sdk-api.md#sub-agents-and-ad-hoc-spaces"],
  ]) {
    const found = lines.find((l) => new RegExp(`\\b${id}\\b`).test(l));
    assert.ok(found, `no decision followed names ${id}`);
    assert.match(found, /\(Carsten, 2026-11-0\d\)/, `${id}: no owner and date`);
    assert.ok(links(found, target), `${id}: no link to ${target}`);
  }
});

test('"Open questions" names the bytes of the associated data of an encrypted previous key, with NOC-110', () => {
  const open = part("open-questions");
  assert.match(open, /associated data/);
  assert.match(open, /previous key/);
  assert.ok(
    targets(open).some((t) =>
      t.includes("linear.app/nocentraldb/issue/NOC-110/"),
    ),
    "no link to NOC-110",
  );
  assert.ok(links(part("key-chain-subkey"), "#open-questions"));
});

test('"History" lists each change by date, oldest first, with its pull request', () => {
  const lines = part("history")
    .split("\n")
    .filter((l) => l.startsWith("- **"));
  const dates = lines.map(
    (l) => l.match(/^- \*\*(\d{4}-\d{2}-\d{2})\*\*/)?.[1],
  );
  assert.ok(dates.every(Boolean), "a History line has no date");
  assert.deepEqual([...dates].sort(), dates, "History is not oldest first");
  for (const l of lines) {
    assert.match(
      l,
      /\]\(https:\/\/github\.com\/nocentraldb\/nocentraldb\/pull\/\d+\)/,
      `no pull request: ${l.slice(0, 60)}`,
    );
  }
  for (const ticket of [
    "NOC-7",
    "NOC-30",
    "NOC-44",
    "NOC-75",
    "NOC-106",
    "NOC-100",
  ]) {
    assert.ok(
      lines.some((l) => new RegExp(`\\b${ticket}\\b`).test(l)),
      `History does not name ${ticket}`,
    );
  }
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

test("NOC-106's answers stand where they belong, linked to their decisions", () => {
  const member = part("the-member-key");
  assert.match(member, /key_file/);
  assert.match(member, /file permissions/);
  assert.ok(links(member, "#16-where-the-member-key-lives"));
  const account = part("the-account-key");
  assert.match(account, /each database draws its own account key once/i);
  assert.ok(links(account, "#17-the-account-key-before-the-accounts-rfc"));
  const subkey = part("key-chain-subkey");
  assert.match(
    subkey,
    /nonce \(24 bytes\) \|\| ciphertext \(32 bytes\) \|\| tag \(16 bytes\)\s+72 bytes/,
  );
  assert.ok(links(subkey, "#18-the-bytes-of-an-encrypted-previous-key"));
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

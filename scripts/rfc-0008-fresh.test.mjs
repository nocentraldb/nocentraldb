// Holds RFC 0008 as a fresh text (NOC-165). The owner wants every RFC to read
// as if it had just been written (Carsten, 2026-11-10): each choice stands in
// the section it belongs to, with a sentence on why and the options not taken,
// and there is no list of decisions, rejected options, conflicts or open
// questions, no reference such as "M13", "D11", "R6" or "decision 15", and no
// History of the drafts. The tests below pin the content of every former
// decision M1 to M29, every row of the former "Rejected options" and every
// former conflict where it now stands, and RFC 0010's changes to RFC 0008
// that were written in with this rewrite (NOC-132): a member checks the chain
// from the recovery key to the certificate, and accounts are compared by
// their recovery key. Whether the wording is right is the review of the PR.
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { join, normalize } from "node:path";
import { test } from "node:test";
import { headings, read, root, section, slug } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0008-membership.md";
const DISCUSSION = "https://github.com/nocentraldb/nocentraldb/discussions/148";
const PUBLISHED =
  "2026-11-10 · First published (RFC 0001 to RFC 0010 rewritten as fresh texts, NOC-165)";

const part = (anchor) => section(read(RFC), anchor);

const links = (text, target) =>
  [...text.matchAll(/\]\(([^)\s]+)\)/g)].some((m) => m[1].endsWith(target));

const line = (text, start) => {
  const found = text.split("\n").find((l) => l.startsWith(start));
  assert.ok(found, `no line starts with "${start}"`);
  return found;
};

const holds = (text, patterns, where) => {
  for (const pattern of patterns) {
    assert.match(text, pattern, `${where} does not say ${pattern}`);
  }
};

const documents = () => [
  "README.md",
  "CONTRIBUTING.md",
  ...readdirSync(join(root, "docs"), { recursive: true })
    .filter((file) => file.endsWith(".md"))
    .map((file) => normalize(join("docs", file))),
];

test("RFC 0008 has no list of decisions, rejected options, conflicts or open questions", () => {
  const found = headings(read(RFC)).map(slug);
  for (const gone of [
    "decisions",
    "rejected-options",
    "conflicts-with-genesismd-and-the-other-rfcs",
    "open-questions",
  ]) {
    assert.ok(!found.includes(gone), `the section #${gone} is left`);
  }
  assert.ok(
    !found.some((anchor) => /^m\d+-/.test(anchor)),
    "a decision heading such as M1 is left",
  );
});

test("RFC 0008 names no decision by its id, its own or another RFC's", () => {
  const text = read(RFC).replace(part("history"), "");
  for (const pattern of [
    /\bM\d+\b/,
    /\bD\d+\b/,
    /\bR\d+\b/,
    /\bS\d+\b/,
    /\bdecisions? \d/i,
    /\bdraft 0\.\d/,
    /\(named in the rewrite/,
  ]) {
    const hit = text.match(pattern);
    assert.ok(!hit, `RFC 0008 still names "${hit?.[0]}"`);
  }
});

test("no document links a removed anchor of RFC 0008", () => {
  const removed =
    /0008-membership\.md#(m\d+-|decisions|rejected-options|conflicts-with-genesismd-and-the-other-rfcs|open-questions)/;
  const found = documents().filter((path) => removed.test(read(path)));
  assert.deepEqual(found, []);
});

test("the status, the discussion and History", () => {
  assert.equal(
    line(read(RFC), "**Status.**"),
    "**Status.** Decided by the owner (Carsten, 2026-11-10).",
  );
  const feedback = line(read(RFC), "**How to give feedback.**");
  assert.ok(links(feedback, DISCUSSION), "the discussion #148 is not linked");
  assert.ok(links(feedback, "README.md#giving-feedback"));
  const lines = part("history")
    .split("\n")
    .filter((l) => l.trim());
  assert.deepEqual(lines, [`- ${PUBLISHED}`]);
});

// Each former decision stands in its section. The comment names the decision
// as the old text numbered it, so the PR's mapping table can be checked
// against this file.

test("the genesis: derived space ids, two geneses, and the model id sealed", () => {
  // M7, M8, M24, M26; rejected: a random space id
  holds(
    part("what-it-names"),
    [
      /128 random bits, and the genesis with the lower change id wins/,
      /model id out of the genesis/,
    ],
    "What it names",
  );
  holds(
    part("how-the-relay-and-members-recognise-it"),
    [/[Bb]oth are rejected, and the space is unusable/],
    "How the relay and members recognise it",
  );
  holds(
    part("genesis"),
    [
      /16 raw bytes of `model\.id`, readable by the relay/,
      /`H\(space id \|\| model\.id\)`/,
    ],
    "Genesis",
  );
});

test("roles: three per space, the sign-off, a space without an admin, an agent's role", () => {
  // M1, M9, M13, M14; rejected: capability lists, rights per collection, a quorum
  holds(
    part("roles-and-rights"),
    [
      /[Ww]riter and admin only/,
      /capabilities per member/,
      /[Rr]ights per collection or per object/,
      /quorum of admins/,
      /any account with a member in the space/i,
      /most senior writer becomes admin/,
      /[Rr]eader, unless the app asks for more/,
      /[Nn]o default/,
    ],
    "Roles and rights",
  );
});

test("an account is its recovery key, and a certificate is checked through its statement", () => {
  // NOC-132
  holds(part("roles-and-rights"), [/recovery key/], "Roles and rights");
  holds(
    part("what-this-rfc-uses-from-rfc-0003"),
    [/recovery key/, /account key statement/, /chain/],
    "What this RFC uses from RFC 0003",
  );
});

test("the control bodies: one entry type with an action, and no clock", () => {
  // M18, M22
  holds(
    part("the-control-bodies"),
    [/hybrid logical clock/, /[Oo]ne entry type per action/],
    "The control bodies",
  );
});

test("a re-grant names a fresh generation, and a member id can come back", () => {
  // M5; rejected: a re-grant without a fresh generation, never re-adding
  holds(
    part("membership-entry"),
    [/[Nn]ever re-adding a member id/, /without a fresh generation/],
    "Membership entry",
  );
});

test("generation ids, two rotations with one id, and rejected rotations", () => {
  // M6, M17, M23; rejected: a random tag, rejecting shared ids without deriving
  holds(
    part("generation-ids"),
    [
      /[Gg]eneration numbers only, with the lower key chain hash winning/,
      /32-byte generation id/,
      /random tag/,
      /without deriving it/,
      /first in the order keeps the id/,
      /rejected with the generation/,
    ],
    "Generation ids",
  );
});

test("the fold: what it reads and the order it computes", () => {
  // rejected: the order of arrival, fold rules that check wrapped keys, a total order as MLS commits
  holds(
    part("folding-the-membership-state"),
    [
      /order of arrival, or the relay's order/,
      /check wrapped keys or decryption/,
      /MLS commits/,
    ],
    "Folding the membership state",
  );
});

test("seniority, verdicts that change, and the cycles not taken", () => {
  // M4; rejected: both or neither of two mutual removals, seniority per causal past
  holds(
    part("two-admins-who-remove-each-other"),
    [/[Bb]oth of two mutual removals/, /[Nn]either of two mutual removals/],
    "Two admins who remove each other",
  );
  holds(part("the-function"), [/own causal past/], "The function");
  holds(
    part("what-a-new-change-can-change"),
    [/stolen device wrote meanwhile/],
    "What a new change can change",
  );
});

test("the cutoff, and the rules for revoked members not taken", () => {
  // M3; rejected: dependencies inside sealed changes, R7 A, revoking by time
  holds(
    part("the-cutoff"),
    [
      /hash over the subject's change ids/,
      /relay accepted before the revocation/,
      /dependencies inside sealed changes/,
      /[Rr]evoking by time/,
    ],
    "The cutoff",
  );
});

test("a shred is final when accepted; waiting until it is stable was not taken", () => {
  // The owner's NOC-224, option 6; rejected: the wait (M20, M21), refusing a
  // fork under a counted sequence, an equivocation as a revocation
  const final = part("a-shred-is-final-when-accepted");
  holds(
    final,
    [
      /Carsten, 2026-11-11, NOC-224/,
      /only its causal past/,
      /[Nn]o revocation excludes a shred/,
      /sign-off is the whole defence/,
      /\*\*Waiting until the shred is stable was not taken\*\*/,
      /equivocation/,
      /NOC-226/,
    ],
    "A shred is final when accepted",
  );
  assert.doesNotMatch(final, /7 days|alarm/, "the alarm is gone");
  const rfc = read(RFC);
  assert.doesNotMatch(rfc, /effects-that-wait-until-a-change-is-stable/);
  assert.doesNotMatch(rfc, /\| Stable \|/, "the term Stable is gone");
  assert.match(rfc, /\| M-C8 \| An accepted shred stays accepted/);
  assert.match(
    rfc,
    /a revocation that excludes a shred/,
    "M-C12 has no fixture for it",
  );
});

test("the cutoff says a revoked admin's fork can depend on its change id", () => {
  // NOC-226's finding
  holds(
    part("the-cutoff"),
    [/change id/, /NOC-226/, /holds the right in the state so far/],
    "The cutoff",
  );
});

test("a grant concurrent with a revocation loses", () => {
  // M2; rejected: add wins
  holds(
    part("concurrent-add-and-remove-of-one-member"),
    [/[Tt]he grant wins/, /lost removal leaves a stolen device writing/],
    "Concurrent add and remove of one member",
  );
});

test("the current generation, who may hold one, and automatic merge rotations", () => {
  // M15, M27, M28, M29
  holds(
    part("two-removals-that-rotate-the-key-at-once"),
    [
      /app asks an admin to rotate/,
      /[Ll]etting it be current/,
      /[Ll]eaving rejected rotations out of the heads/,
      /[Cc]ounting only accepted envelopes/,
      /force rotations by naming the current generation/,
    ],
    "Two removals that rotate the key at once",
  );
});

test("the relay applies the same rules, pushes again, and what it learns", () => {
  // M11, M19; rejected: the relay's table as the truth, hiding control bodies
  holds(
    part("how-the-relay-applies-the-rules"),
    [
      /relay's membership table as the truth/,
      /[Hh]iding the control bodies/,
      /never push it to that relay again/,
      /account key weighs most/,
    ],
    "How the relay applies the rules",
  );
});

test("a sub-agent's certificate is signed by its parent", () => {
  // M25
  holds(
    part("what-this-rfc-uses-from-rfc-0003"),
    [/account key, as for every other kind/],
    "What this RFC uses from RFC 0003",
  );
});

test("groups stay out, and compaction keeps every control change", () => {
  // M10, M16
  holds(
    part("groups"),
    [
      /withdrawn by the owner \(Carsten, 2026-11-09\)/,
      /principal with its own control state/,
    ],
    "Groups",
  );
  holds(
    part("what-this-rfc-does-not-cover"),
    [/signed snapshot/, /under 1 MB/],
    "What this RFC does not cover",
  );
});

test("where this RFC makes the others more precise, each linked where the other text states it", () => {
  // the former conflicts 1 to 16
  const agreement = part("agreement-with-genesismd-and-the-other-rfcs");
  for (const target of [
    "0001-nocentraldb.md#groups-leave-the-design",
    "0002-change-and-log-format.md#what-the-relay-can-see-and-what-it-cannot",
    "0002-change-and-log-format.md#space",
    "0002-change-and-log-format.md#key-generation-suite-and-kind",
    "0002-change-and-log-format.md#author-and-author-sequence",
    "0002-change-and-log-format.md#a-change-the-relay-refuses-after-the-member-folded-it",
    "0003-keys-and-cryptography.md#removing-a-member-and-rotating-the-space-key",
    "0003-keys-and-cryptography.md#shredding-a-persons-key",
    "0003-keys-and-cryptography.md#the-person-key",
    "0007-relay.md#what-the-relay-checks",
    "0006-local-view.md#control-changes-and-changes-that-wait-for-them",
    "0003-keys-and-cryptography.md#model-id-subkey",
  ]) {
    assert.ok(
      links(agreement, target),
      `the agreement does not link ${target}`,
    );
  }
  assert.doesNotMatch(agreement, /\*Resolved/);
});

test("the entry that carries a newer account key statement is named and linked", () => {
  // NOC-132, then the owner's NOC-187 (Carsten, 2026-11-11): the entry is
  // designed, so the text links it instead of naming it as still to come.
  holds(
    part("what-this-rfc-uses-from-rfc-0003"),
    [/NOC-187/, /\(#account-key-statement-entry\)/, /older account key/],
    "What this RFC uses from RFC 0003",
  );
  assert.doesNotMatch(part("what-this-rfc-uses-from-rfc-0003"), /still to be designed/);
});

test("the first accepted person-key entry chooses the key, and what that does not protect", () => {
  // NOC-133 point 1
  const entry = part("person-key-entry");
  assert.match(
    entry,
    /first accepted person-key entry in the order creates the person key/,
  );
  assert.match(entry, /choose the key/);
  assert.match(entry, /rejected as "commitment"/);
  assert.match(
    entry,
    /a shred never protects against a member who copied the data/,
  );
});

test("the model id in every change's payload is named as RFC 0002's and RFC 0005's", () => {
  // the former conflict 14's scope note
  assert.match(
    part("agreement-with-genesismd-and-the-other-rfcs"),
    /model id in every change's payload is RFC 0002's and RFC 0005's to carry out/,
  );
});

test("the key chain entry states its rule without the owner's attribution", () => {
  assert.doesNotMatch(part("key-chain-entry"), /as the owner decided/);
});

test("the sign-off: another account's writer when the performer's account is the only admin account", () => {
  // The owner's NOC-228, question 2 A
  for (const anchor of ["roles-and-rights", "shred-entry"]) {
    holds(
      part(anchor),
      [
        /Carsten, 2026-11-11, NOC-228/,
        /exactly one account/,
        /writer member/,
        /[Rr]eaders? may not|not a reader/,
      ],
      anchor,
    );
  }
});

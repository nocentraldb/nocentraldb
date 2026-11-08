// Asserts that the texts a user reads first say everything the relay can see
// and every limit of erasure that RFC 0002 and RFC 0003 established (NOC-20):
// RFC 0001, the FAQ, the use cases and GENESIS.md name the limits or link to
// the RFC section that lists them, and every relative link in those files
// resolves to a file, and to a heading when it names one. The test checks
// that each claim names its limit and points to its source; whether the
// wording is right is the owner's review of the pull request.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

const RFC_0001 = "docs/rfc/0001-nocentraldb.md";
const FAQ = "docs/faq.md";
const USE_CASES = "docs/use-cases.md";
const GENESIS = "factory/GENESIS.md";
const RFC_0003 = "docs/rfc/0003-keys-and-cryptography.md";

const RELAY_SEES =
  "0002-change-and-log-format.md#what-the-relay-can-see-and-what-it-cannot";
const SHREDDING = "0003-keys-and-cryptography.md#shredding-a-persons-key";
const REMOVING =
  "0003-keys-and-cryptography.md#removing-a-member-and-rotating-the-space-key";
const RFC_0003_DECISIONS =
  "0003-keys-and-cryptography.md#decisions-for-the-owner";
const RFC_0003_SDK = "0003-keys-and-cryptography.md#how-the-sdk-hides-keys";
const PERSON_KEY = "0003-keys-and-cryptography.md#the-person-key";

/** GitHub's heading anchor: lower case, punctuation dropped, spaces to dashes. */
const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .replace(/\s/g, "-");
const anchors = (markdown) =>
  new Set(
    markdown
      .split("\n")
      .filter((line) => /^#{1,6} /.test(line))
      .map((line) => slug(line.replace(/^#+ /, ""))),
  );

/** The text under the heading `heading` (any level), up to the next heading of the same or a higher level. */
const section = (markdown, heading) => {
  const lines = markdown.split("\n");
  const start = lines.findIndex((line) =>
    new RegExp(
      `^#{1,6} ${heading.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`,
    ).test(line),
  );
  assert.ok(start >= 0, `no heading "${heading}"`);
  const level = lines[start].match(/^#+/)[0].length;
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((line) => {
    const m = line.match(/^(#+) /);
    return m && m[1].length <= level;
  });
  return (end < 0 ? rest : rest.slice(0, end)).join("\n");
};

/** The bullet of a list that starts with `**<label>`. */
const bullet = (markdown, label) => {
  const line = markdown.split("\n").find((l) => l.startsWith(`- **${label}`));
  assert.ok(line, `no bullet "${label}"`);
  return line;
};

/** True if `text` holds a Markdown link whose target ends with `target`. */
const links = (text, target) =>
  [...text.matchAll(/\]\(([^)\s]+)\)/g)].some(([, t]) => t.endsWith(target));

test("RFC 0001 says the relay sees more than its summary names, and links to RFC 0002's list", () => {
  const relay = bullet(read(RFC_0001), "A relay that holds no keys.");
  assert.ok(
    links(relay, RELAY_SEES),
    "the relay bullet does not link RFC 0002",
  );
  assert.match(
    relay,
    /withhold/i,
    "the relay bullet does not say it can withhold changes",
  );
});

test("the FAQ says the relay sees more than keys and write volume, and links to RFC 0002's list", () => {
  const answer = section(read(FAQ), "What can the relay see?");
  assert.ok(
    links(answer, `rfc/${RELAY_SEES}`),
    "the answer does not link RFC 0002",
  );
  for (const item of [
    /rhythm/i,
    /network address/i,
    /generation/i,
    /withhold/i,
    /different subsets/i,
  ]) {
    assert.match(answer, item, `the answer does not name ${item}`);
  }
});

test("RFC 0001's limits of erasure include the two that RFC 0003 adds, with a link", () => {
  const rfc = read(RFC_0001);
  const teamLead = section(
    rfc,
    "The team lead who has to pass the data-protection review",
  );
  const shred = bullet(rfc, "Shred and audit.");
  for (const [where, text] of [
    ["the team lead", teamLead],
    ["Shred and audit", shred],
  ]) {
    assert.match(text, /modified client/i, `${where}: no modified client`);
    assert.match(text, /backup/i, `${where}: no relay backup`);
    assert.ok(
      links(text, SHREDDING),
      `${where}: no link to RFC 0003's shredding`,
    );
  }
});

test("RFC 0001 says v1 is not post-quantum, with a link to RFC 0003", () => {
  const rfc = read(RFC_0001);
  const line = rfc.split("\n").find((l) => /post-quantum/i.test(l));
  assert.ok(line, "RFC 0001 does not mention post-quantum");
  assert.match(line, /not post-quantum/i);
  assert.ok(links(line, RFC_0003_DECISIONS), "no link to RFC 0003's decisions");
});

test("RFC 0001 says a browser can evict its copy, and what the member fetches again", () => {
  const rfc = read(RFC_0001);
  const line = rfc.split("\n").find((l) => /evict/i.test(l));
  assert.ok(line, "RFC 0001 does not mention eviction");
  assert.match(line, /Safari/);
  assert.match(line, /seven days/i);
  assert.match(
    line,
    /again from the relay/i,
    "does not say what is fetched again",
  );
  assert.match(line, /lost/i, "does not say unconfirmed changes are lost");
  assert.ok(
    links(line, "0006-local-view.md#the-browser"),
    "the eviction bullet does not link RFC 0006, The browser",
  );
});

test("the FAQ's GDPR answer names the two limits that RFC 0003 adds, with a link", () => {
  const answer = section(
    read(FAQ),
    "Does destroying a key satisfy a GDPR erasure request?",
  );
  assert.match(answer, /modified client/i);
  assert.match(answer, /backup/i);
  assert.ok(
    links(answer, `rfc/${SHREDDING}`),
    "no link to RFC 0003's shredding",
  );
});

test("GENESIS.md: one key per person per space, the two erasure limits, and not post-quantum", () => {
  const genesis = read(GENESIS);
  const shred = bullet(genesis, "Shred and audit.");
  assert.match(shred, /one key per person per space/i);
  assert.match(shred, /modified client/i);
  assert.match(shred, /backup/i);
  assert.ok(
    links(shred, `docs/rfc/${PERSON_KEY}`),
    "Shred and audit: no link to RFC 0003's person key",
  );
  assert.ok(
    links(shred, `docs/rfc/${SHREDDING}`),
    "Shred and audit: no link to RFC 0003's shredding",
  );
  const promise = genesis
    .split("\n")
    .find((l) => l.startsWith("- Never claim more than the design delivers."));
  assert.ok(promise, "no promise 'Never claim more than the design delivers.'");
  assert.match(promise, /modified client/i);
  assert.match(promise, /backup/i);
  assert.match(promise, /not post-quantum/i);
  assert.ok(
    links(promise, `docs/rfc/${SHREDDING}`),
    "promise: no link to RFC 0003's shredding",
  );
  assert.ok(
    links(promise, `docs/rfc/${RFC_0003_DECISIONS}`),
    "promise: no link to RFC 0003's decisions",
  );
});

test("use case 1 names the offline-device limit where it claims every copy is unreadable", () => {
  const uc1 = section(read(USE_CASES), "1. Erasure that reaches every copy");
  const line = uc1.split("\n").find((l) => /(is|are) unreadable/.test(l));
  assert.ok(line, "use case 1 no longer says the copies are unreadable");
  assert.match(line, /never comes? back online/i);
  assert.ok(links(line, `rfc/${SHREDDING}`), "no link to RFC 0003's shredding");
});

test("use case 3 says members that have not seen the removal may still write under the old generation", () => {
  const uc3 = section(
    read(USE_CASES),
    "3. An outside agent limited to one space",
  );
  assert.ok(
    !/receives no new changes after that\./.test(uc3),
    "the unqualified claim remains",
  );
  assert.match(uc3, /old (key )?generation/i);
  assert.ok(links(uc3, `rfc/${REMOVING}`), "no link to RFC 0003's removal");
});

test("use case 3 adds a member by invitation and removes it by member id, never by public key", () => {
  const uc3 = section(
    read(USE_CASES),
    "3. An outside agent limited to one space",
  );
  assert.ok(!/PublicKey/.test(uc3), "use case 3 still passes a public key");
  assert.match(uc3, /members\.add\(invitation/);
  assert.match(uc3, /members\.remove\(memberId\)/);
  assert.ok(
    links(uc3, `rfc/${RFC_0003_SDK}`),
    "no link to RFC 0003's SDK section",
  );
});

test("every relative link in RFC 0001, the FAQ, the use cases, GENESIS.md and RFC 0003 resolves, anchors included", () => {
  for (const path of [RFC_0001, FAQ, USE_CASES, GENESIS, RFC_0003]) {
    const text = read(path).replace(/```[\s\S]*?```/g, "");
    for (const [, target] of text.matchAll(/\]\(([^)\s]+)\)/g)) {
      if (/^[a-z]+:/.test(target)) continue;
      const [file, anchor] = target.split("#");
      const resolved = file ? normalize(join(dirname(path), file)) : path;
      assert.ok(existsSync(join(root, resolved)), `${path}: ${target}`);
      if (anchor && resolved.endsWith(".md")) {
        assert.ok(
          anchors(read(resolved)).has(anchor),
          `${path}: no heading #${anchor} in ${resolved}`,
        );
      }
    }
  }
});

test("GENESIS.md's relay bullet says the relay sees more, and links to RFC 0002's list", () => {
  const relay = bullet(read(GENESIS), "A relay that holds no keys.");
  assert.ok(
    links(relay, `docs/rfc/${RELAY_SEES}`),
    "the relay bullet does not link RFC 0002",
  );
  assert.match(
    relay,
    /withhold/i,
    "the relay bullet does not say it can withhold changes",
  );
});

test("use case 2 says the relay sees more, and links to RFC 0002's list", () => {
  const uc2 = section(
    read(USE_CASES),
    "2. A breach of the relay leaks nothing readable",
  );
  const limits = uc2
    .split("\n")
    .find((l) => l.startsWith("**What it does not cover.**"));
  assert.ok(limits, "use case 2 has no 'What it does not cover'");
  assert.ok(
    links(limits, `rfc/${RELAY_SEES}`),
    "use case 2 does not link RFC 0002",
  );
  assert.match(
    limits,
    /withhold/i,
    "use case 2 does not say the relay can withhold changes",
  );
});

test("RFC 0001 never says one key per person without 'per space'", () => {
  const rfc = read(RFC_0001);
  for (const label of ["Shred and audit.", "Personal fields."]) {
    const text = bullet(rfc, label);
    assert.match(
      text,
      /one key per person per space/i,
      `${label} lacks 'one key per person per space'`,
    );
    assert.ok(
      links(text, PERSON_KEY),
      `${label} does not link RFC 0003's person key`,
    );
  }
  assert.ok(
    !/one key per person(?! per space)/i.test(rfc),
    "RFC 0001 still says 'one key per person' without 'per space'",
  );
});

test("use case 1's list of what it does not cover names the two limits that RFC 0003 adds", () => {
  const uc1 = section(read(USE_CASES), "1. Erasure that reaches every copy");
  const list = uc1
    .split("**What it does not cover.**")[1]
    ?.split("**Tell us.**")[0];
  assert.ok(list, "use case 1 has no 'What it does not cover'");
  assert.match(
    list,
    /^- .*modified client/im,
    "no bullet for a modified client",
  );
  assert.match(list, /^- .*backup/im, "no bullet for a relay backup");
  assert.ok(links(list, `rfc/${SHREDDING}`), "no link to RFC 0003's shredding");
});

test("RFC 0003 no longer says the use cases pass a public key to members.add", () => {
  // The accepted reading 4 in "Agreement with GENESIS.md and RFC 0001" keeps
  // naming agentPublicKey as the record of what it changed; only the SDK
  // section's present-tense note must follow the use case.
  const sdk = section(read(RFC_0003), "How the SDK hides keys");
  assert.ok(
    !/agentPublicKey/.test(sdk),
    "RFC 0003's SDK section still names agentPublicKey",
  );
  assert.ok(
    links(sdk, "use-cases.md#3-an-outside-agent-limited-to-one-space"),
    "RFC 0003's SDK section does not link use case 3",
  );
});

// Owner review of #45: each passage states what holds and its limits once, as
// one statement; no sentence is taken back by a later one.
const SELF_CORRECTION =
  /that is not all|more than that|however, it also|after all/i;

test("no passage in RFC 0001, the FAQ, the use cases, GENESIS.md or RFC 0003 corrects itself", () => {
  for (const path of [RFC_0001, FAQ, USE_CASES, GENESIS, RFC_0003]) {
    const line = read(path)
      .split("\n")
      .find((l) => SELF_CORRECTION.test(l));
    assert.ok(!line, `${path} corrects itself: ${line?.slice(0, 120)}`);
  }
});

test("erasure is never claimed for every device or everywhere, only where the key is gone", () => {
  const gdpr = section(
    read(FAQ),
    "Does destroying a key satisfy a GDPR erasure request?",
  );
  assert.ok(
    !/on every device(?! that)/i.test(gdpr),
    "the FAQ claims every device",
  );
  const personal = bullet(read(RFC_0001), "Personal fields.");
  assert.ok(
    !/everywhere/i.test(personal),
    "RFC 0001's Personal fields claims everywhere",
  );
  const uc1 = section(read(USE_CASES), "1. Erasure that reaches every copy");
  const claim = uc1.split("\n").find((l) => /(is|are) unreadable/.test(l));
  assert.ok(claim, "use case 1 no longer says the copies are unreadable");
  assert.ok(
    !/\. A device that never comes back online keeps the key/.test(claim),
    "use case 1 takes its claim back in a later sentence",
  );
});

test("RFC 0001's Shred and audit names its limits within the erasure statement", () => {
  const shred = bullet(read(RFC_0001), "Shred and audit.");
  assert.ok(
    !/Erasure cannot take back/.test(shred),
    "the limits follow as a separate correction",
  );
});

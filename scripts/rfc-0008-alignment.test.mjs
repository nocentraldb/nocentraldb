// Asserts that the RFCs on main follow RFC 0008's decisions (NOC-44): RFC
// 0001, 0002, 0003 and 0006 each carry an amendment that a dated changelog
// line at the top announces, every point of that amendment links the RFC 0008
// decision or section it follows, the FAQ lists what the relay sees of
// control changes, no limit of erasure is stated more weakly than RFC 0008
// states it, and RFC 0008's "Conflicts" section says which of its points are
// resolved. The test checks that each claim is there and points to its
// source; whether the wording is right is the owner's review of the PR.
import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

const RFC_0001 = "docs/rfc/0001-nocentraldb.md";
const RFC_0002 = "docs/rfc/0002-change-and-log-format.md";
const RFC_0003 = "docs/rfc/0003-keys-and-cryptography.md";
const RFC_0006 = "docs/rfc/0006-local-view.md";
const RFC_0008 = "docs/rfc/0008-membership.md";
const FAQ = "docs/faq.md";
const USE_CASES = "docs/use-cases.md";
const DECISIONS = "factory/decisions";

const RFC_0008_FILE = "0008-membership.md";
const STABLE = `${RFC_0008_FILE}#effects-that-wait-until-a-change-is-stable`;
const RELAY_RULES = `${RFC_0008_FILE}#how-the-relay-applies-the-rules`;

/** Each amended RFC, the number of its new amendment, and the RFC 0008 decisions it must name. */
const AMENDMENTS = [
  { path: RFC_0001, number: 1, decisions: ["M10", "M11", "M20", "M21"] },
  {
    path: RFC_0002,
    number: 3,
    decisions: ["M3", "M6", "M7", "M11", "M18", "M19", "M24"],
  },
  { path: RFC_0003, number: 2, decisions: ["M6", "M9", "M15", "M20", "M26"] },
  { path: RFC_0006, number: 1, decisions: ["M4", "M20"] },
];
const anchorOf = (number) => `amendment-${number}-following-rfc-0008`;

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

/** The text under the heading whose anchor is `anchor`, up to the next heading of the same or a higher level. */
const section = (markdown, anchor) => {
  const lines = markdown.split("\n");
  const start = lines.findIndex(
    (line) =>
      /^#{1,6} /.test(line) && slug(line.replace(/^#+ /, "")) === anchor,
  );
  if (start < 0) return undefined;
  const level = lines[start].match(/^#+/)[0].length;
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((line) => {
    const m = line.match(/^(#+) /);
    return m && m[1].length <= level;
  });
  return (end < 0 ? rest : rest.slice(0, end)).join("\n");
};

/** The Markdown link targets in `text`. */
const targets = (text) =>
  [...text.matchAll(/\]\(([^)\s]+)\)/g)].map((m) => m[1]);
const links = (text, target) => targets(text).some((t) => t.endsWith(target));

/** The relative links of a Markdown file outside code blocks, resolved. */
const relativeLinks = (path) =>
  targets(read(path).replace(/```[\s\S]*?```/g, ""))
    .filter((target) => !/^[a-z]+:/.test(target))
    .map((target) => {
      const [file, anchor] = target.split("#");
      const resolved = file ? normalize(join(dirname(path), file)) : path;
      return { target, resolved, anchor };
    });

/** True if `target`, relative to `from`, names a heading of RFC 0008 that exists. */
const resolvesInto0008 = (from, target) => {
  const [file, anchor] = target.split("#");
  if (normalize(join(dirname(from), file)) !== RFC_0008 || !anchor) {
    return false;
  }
  return anchors(read(RFC_0008)).has(anchor);
};

for (const { path, number, decisions } of AMENDMENTS) {
  const anchor = anchorOf(number);

  test(`${path} announces Amendment ${number} in a dated NOC-44 changelog line at the top`, () => {
    const top = read(path).split(/^## /m)[0];
    assert.match(
      top,
      new RegExp(
        `^- \\*\\*\\d{4}-\\d{2}-\\d{2}\\*\\* · NOC-44 · .*\\(#${anchor}\\)`,
        "m",
      ),
      `no dated changelog line for NOC-44 linking to #${anchor} above the first section`,
    );
  });

  test(`${path} has the section of Amendment ${number}, and every point links RFC 0008`, () => {
    const body = section(read(path), anchor);
    assert.ok(body, `no section #${anchor}`);
    const points = body.split("\n").filter((line) => line.startsWith("- **"));
    assert.ok(points.length > 0, "the amendment lists no points");
    for (const point of points) {
      assert.ok(
        targets(point).some((t) => resolvesInto0008(path, t)),
        `a point links no heading of RFC 0008: ${point.slice(0, 100)}`,
      );
    }
    for (const id of decisions) {
      assert.match(
        body,
        new RegExp(`\\b${id}\\b`),
        `the amendment does not name ${id}`,
      );
    }
  });
}

test("every relative link in the amended RFCs, RFC 0008 and the FAQ resolves, anchors included", () => {
  for (const path of [RFC_0001, RFC_0002, RFC_0003, RFC_0006, RFC_0008, FAQ]) {
    for (const { target, resolved, anchor } of relativeLinks(path)) {
      assert.ok(existsSync(join(root, resolved)), `${path}: ${target}`);
      if (!anchor || !resolved.endsWith(".md")) continue;
      assert.ok(
        anchors(read(resolved)).has(anchor),
        `${path}: ${target} names no heading`,
      );
    }
  }
});

// RFC 0008's M10 had RFC 0001 define a group as a list the SDK expands; the
// owner withdrew M10 and groups altogether (Carsten, 2026-11-09, NOC-76), so
// the "Spaces" bullet now defines a member and names no group at all.
test("RFC 0001 no longer says groups can be members, and its Spaces bullet names no group", () => {
  const rfc = read(RFC_0001);
  assert.ok(
    !/Groups can be members of many spaces/.test(rfc),
    "RFC 0001 still says groups can be members of many spaces",
  );
  const spaces = rfc.split("\n").find((l) => l.startsWith("- **Spaces.**"));
  assert.ok(spaces, "no bullet 'Spaces.'");
  assert.match(
    spaces,
    /A member is a person's device, an agent or a sub-agent/,
  );
  assert.ok(!/\bgroups?\b/i.test(spaces), "the bullet still names groups");
});

test("the FAQ lists what the relay sees of control changes, with a link to RFC 0008", () => {
  const answer = section(read(FAQ), "what-can-the-relay-see");
  assert.ok(answer, "no FAQ answer 'What can the relay see?'");
  for (const item of [
    /control change/i,
    /role/i,
    /cutoff/i,
    /builds on/i,
    /account/i,
  ]) {
    assert.match(answer, item, `the answer does not name ${item}`);
  }
  assert.ok(links(answer, RELAY_RULES), "no link to RFC 0008's relay rules");
});

test("the FAQ no longer offers groups as members of many spaces", () => {
  const answer = section(read(FAQ), "how-fine-grained-are-the-rights");
  assert.ok(answer, "no FAQ answer 'How fine-grained are the rights?'");
  assert.ok(!/groups that are members of many spaces/i.test(answer));
});

// Principle 4: RFC 0008 says the key is destroyed only once the shred is
// stable, which an admin device that does not come online delays, and that
// the SDK then raises an alarm. Every list of what erasure does not reach
// names that wait and links RFC 0008.
for (const [path, heading] of [
  [RFC_0001, "the-team-lead-who-has-to-pass-the-data-protection-review"],
  [FAQ, "does-destroying-a-key-satisfy-a-gdpr-erasure-request"],
  [RFC_0003, "shredding-a-persons-key"],
  [USE_CASES, "1-erasure-that-reaches-every-copy"],
]) {
  test(`${path} names the wait until a shred is stable among the limits of erasure`, () => {
    const text = section(read(path), heading);
    assert.ok(text, `no section #${heading}`);
    const line = text
      .split("\n")
      .find((l) => /stable/.test(l) && links(l, STABLE));
    assert.ok(
      line,
      "no passage names the wait and links RFC 0008's stable effects",
    );
    assert.match(line, /admin/i, "does not say an admin device delays it");
    assert.match(line, /alarm/i, "does not name the alarm");
  });
}

test("RFC 0002's list of what the relay sees names what it sees of control changes", () => {
  const list = section(
    read(RFC_0002),
    "what-the-relay-can-see-and-what-it-cannot",
  );
  assert.ok(list, "no section on what the relay sees");
  assert.match(list, /control dependencies/i);
  assert.match(list, /cutoff/i);
  assert.ok(
    links(list, "#how-the-relay-applies-the-rules"),
    "no link to RFC 0008",
  );
  const ordering = list
    .split("\n")
    .find((l) => l.startsWith("- **The ordering metadata"));
  assert.ok(ordering, "no bullet on the ordering metadata");
  assert.match(
    ordering,
    /sealed change/i,
    "does not restrict the claim to sealed changes",
  );
});

test("RFC 0002's amendments 1 and 2 are decided, D13 as A", () => {
  const rfc = read(RFC_0002);
  assert.ok(
    !/Decision: open, for the owner/.test(rfc),
    "a decision is still open",
  );
  const status = rfc.split("\n").find((l) => l.startsWith("**Status.**"));
  assert.ok(
    !/proposal for the owner/.test(status),
    "the status still calls an amendment a proposal",
  );
  for (const id of ["D10", "D11", "D12", "D13"]) {
    const block = rfc.split(`**${id}.`)[1]?.split(/\n\*\*D\d+\.|\n## /)[0];
    assert.ok(block, `no decision ${id}`);
    assert.match(
      block,
      /Decided: A \(Carsten, 2026-11-08\)/,
      `${id} is not decided as A`,
    );
  }
});

test("RFC 0003 no longer lets a reader's account sign off an erasure, or delete a record at once", () => {
  const rfc = read(RFC_0003);
  assert.ok(
    !/has a certified member in the space/.test(rfc),
    "the sign-off is not narrowed",
  );
  assert.ok(
    !/deletes the person-key record from its store in the same step/.test(rfc),
    "the relay still deletes the record when it appends the shred",
  );
});

test("RFC 0003 names the model-id subkey under the formats of what keys derive", () => {
  const formats = section(
    read(RFC_0003),
    "formats-of-what-keys-sign-and-derive",
  );
  assert.ok(formats, "no section on formats");
  const subkey = section(formats, "model-id-subkey");
  assert.ok(subkey, "no subsection 'Model-id subkey'");
  assert.match(subkey, /"nocentraldb\/model-id"/);
  assert.match(subkey, /generation 1/i);
  assert.match(subkey, /associated data/i);
  assert.match(subkey, /collide/i, "does not say why the label cannot collide");
});

test("RFC 0006's control table holds what RFC 0008 lists, and the shred row waits until stable", () => {
  const rfc = read(RFC_0006);
  const control = rfc
    .split("\n")
    .find((l) => l.startsWith("**The control state.**"));
  assert.ok(control, "no paragraph on the control state");
  for (const item of [
    /genesis/i,
    /person-key entr/i,
    /cutoff/i,
    /grant/i,
    /generation graph/i,
  ]) {
    assert.match(control, item, `the control state does not hold ${item}`);
  }
  const shred = rfc.split("\n").find((l) => l.startsWith("| Shred entry"));
  assert.ok(shred, "no shred row");
  assert.match(shred, /stable/, "the shred row does not wait until stable");
  assert.ok(
    links(shred, STABLE),
    "the shred row does not link RFC 0008's definition",
  );
  assert.match(
    rfc,
    /`resolve`/,
    "control changes are not recomputed through resolve",
  );
});

test("RFC 0008's Conflicts section says which points NOC-44 resolved", () => {
  const conflicts = section(
    read(RFC_0008),
    "conflicts-with-genesismd-and-the-other-rfcs",
  );
  assert.ok(conflicts, "no Conflicts section");
  const points = new Map(
    conflicts
      .split("\n")
      .filter((l) => /^\d+\. /.test(l))
      .map((l) => [Number(l.split(".")[0]), l]),
  );
  for (const n of [1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 13, 14, 15]) {
    const point = points.get(n);
    assert.ok(point, `no conflict ${n}`);
    assert.match(point, /Resolved/, `conflict ${n} is not marked resolved`);
    assert.ok(
      targets(point).some((t) => /#amendment-\d+-following-rfc-0008$/.test(t)),
      `conflict ${n} links no amendment`,
    );
  }
});

test("the decision log records NOC-44's routing and the owner's D13", () => {
  const files = readdirSync(join(root, DECISIONS)).filter((f) =>
    f.includes("-NOC-44-"),
  );
  const texts = files.map((f) => read(join(DECISIONS, f)));
  assert.ok(
    texts.some((t) => /- Decided by: pm/.test(t) && /RFC 0008/.test(t)),
    "no decision by pm routing RFC 0008's decisions",
  );
  assert.ok(
    texts.some((t) => /- Decided by: Carsten/.test(t) && /D13/.test(t)),
    "no decision by Carsten on D13",
  );
});

// The owner decided GENESIS.md follows RFC 0008's M10 (Carsten, 2026-11-08):
// a member is a person's device, an agent or a sub-agent, and GENESIS.md no
// longer offers groups as members. The owner then dropped the sentence on
// groups from GENESIS.md (Carsten, 2026-11-09); RFC 0008 keeps the details.
test("GENESIS.md's Spaces bullet follows M10: members are devices, agents and sub-agents", () => {
  const genesis = read("factory/GENESIS.md");
  assert.ok(
    !/groups can be members of many spaces/i.test(genesis),
    "GENESIS.md still says groups can be members of many spaces",
  );
  const spaces = genesis.split("\n").find((l) => l.startsWith("- **Spaces.**"));
  assert.ok(spaces, "no bullet 'Spaces.'");
  for (const item of [/device/i, /agent/i, /sub-agent/i]) {
    assert.match(spaces, item, `the Spaces bullet does not name ${item}`);
  }
});

test("RFC 0008's conflict 1 is resolved for GENESIS.md too", () => {
  const conflicts = section(
    read(RFC_0008),
    "conflicts-with-genesismd-and-the-other-rfcs",
  );
  const point = conflicts.split("\n").find((l) => l.startsWith("1. "));
  assert.match(point, /Resolved for GENESIS\.md/);
  assert.ok(
    !/stands until the owner rewrites it/.test(conflicts),
    "the intro still says GENESIS.md's sentence stands",
  );
});

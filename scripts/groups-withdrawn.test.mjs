// Asserts that groups have left the design (NOC-76). The owner decided to
// keep groups out of every definition for now (Carsten, 2026-11-09); they may
// return through a later RFC. RFC 0001 records the decision under "Groups
// leave the design" (NOC-100's rfc-0001-complete.test.mjs holds that
// section), and RFC 0008 and RFC 0009 state it in their text since NOC-101
// folded their amendments in; the
// decisions about groups, RFC 0008's M10 and RFC 0009's S14, are marked
// withdrawn instead of deleted; the decision log records the owner's call;
// and no RFC or doc presents groups as part of the design outside the text
// that withdraws them. Whether the wording is right is the owner's review.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

const RFC_0008 = "docs/rfc/0008-membership.md";
const RFC_0009 = "docs/rfc/0009-sdk-api.md";
const DECISIONS = "factory/decisions";

const WITHDRAWING =
  /#(amendment-\d+-)?groups-leave-the-design\)|(0008-membership\.md)?#groups\)/;
const WITHDRAWN = /withdrawn by the owner \(Carsten, 2026-11-09\)/i;

/** The text of the decision `id` (such as "M10"), up to the next decision or heading. */
const decision = (markdown, id) => {
  const lines = markdown.split("\n");
  const start = lines.findIndex(
    (line) =>
      line.startsWith(`**${id}. `) ||
      new RegExp(`^#{1,6} ${id}\\. `).test(line),
  );
  if (start < 0) return undefined;
  const rest = lines.slice(start + 1);
  const end = rest.findIndex(
    (line) => /^\*\*[A-Z]+\d+\. /.test(line) || /^#{1,6} /.test(line),
  );
  return [lines[start], ...(end < 0 ? rest : rest.slice(0, end))].join("\n");
};

/**
 * Uses of the word "group" that are not the withdrawn feature: an auditor's
 * name, a workflow's concurrency group, the cryptographic group key RFC 0003
 * compares protocols for, and plain English.
 */
const OTHER_GROUPS = [
  /NCC Group/g,
  /concurrency group/gi,
  /group key/gi,
  /Signal and WhatsApp groups/g,
  /commits per group/g,
  /fork the group/g,
  /\bgrouping\b/g,
  /a parent only groups/g,
  /older job of the group/g,
];
const mentionsGroups = (text) =>
  /\bgroups?\b/i.test(
    OTHER_GROUPS.reduce((rest, other) => rest.replace(other, ""), text),
  );

/** Every Markdown document a reader takes the design from. */
const documents = () => [
  "README.md",
  "CONTRIBUTING.md",
  ...readdirSync(join(root, "docs"), { recursive: true })
    .filter((file) => file.endsWith(".md") && !file.startsWith("rfc/vectors"))
    .map((file) => normalize(join("docs", file))),
];

/**
 * The units a reader takes as one statement: a decision block with its
 * options, a section headed "Groups" or by an amendment that withdraws
 * groups, and otherwise each line.
 */
const units = (markdown) => {
  const lines = markdown.split("\n");
  const result = [];
  for (let i = 0; i < lines.length; ) {
    const line = lines[i];
    const heading = line.match(/^(#{1,6}) (.*)$/);
    const isDecision = /^(\*\*|#{1,6} )[A-Z]+\d+\. /.test(line);
    const isWhole =
      heading &&
      (heading[2] === "Groups" || /groups leave the design/i.test(heading[2]));
    if (!isDecision && !isWhole) {
      result.push(line);
      i += 1;
      continue;
    }
    let j = i + 1;
    while (
      j < lines.length &&
      !(
        (isDecision && /^(\*\*|#{1,6} )[A-Z]+\d+\. /.test(lines[j])) ||
        (/^(#{1,6}) /.test(lines[j]) &&
          (!isWhole || lines[j].match(/^#+/)[0].length <= heading[1].length))
      )
    ) {
      j += 1;
    }
    result.push(lines.slice(i, j).join("\n"));
    i = j;
  }
  return result;
};

for (const [path, id] of [
  [RFC_0008, "M10"],
  [RFC_0009, "S14"],
]) {
  test(`${path} marks decision ${id} on groups as withdrawn by the owner, and keeps it`, () => {
    const text = decision(read(path), id);
    assert.ok(text, `no decision ${id}`);
    assert.match(text, WITHDRAWN, `${id} is not marked withdrawn`);
    assert.match(text, WITHDRAWING, `${id} does not link the amendment`);
    assert.match(text, /Decided: |Decided \(/, `${id} lost its decision`);
  });
}

test("RFC 0009 keeps adding several invitations in one call as plain batching", () => {
  const rfc = read(RFC_0009);
  const line = rfc
    .split("\n")
    .find((l) => l.startsWith("- **`members.add(invitation, { role })`**"));
  assert.ok(line, "no bullet for members.add");
  assert.match(line, /list of invitations/);
  assert.ok(!mentionsGroups(line), "the bullet still names groups");
});

test("no RFC or doc presents groups as part of the design outside the text that withdraws them", () => {
  const found = [];
  for (const path of documents()) {
    for (const unit of units(read(path))) {
      if (!mentionsGroups(unit)) continue;
      if (WITHDRAWING.test(unit) || WITHDRAWN.test(unit)) continue;
      if (/^#+ .*groups leave the design/im.test(unit)) continue;
      found.push(`${path}: ${unit.split("\n")[0].slice(0, 120)}`);
    }
  }
  assert.deepEqual(found, [], "groups still presented as design");
});

test("the decision log records the owner's decision that groups leave the design", () => {
  const files = readdirSync(join(root, DECISIONS)).filter((file) =>
    file.startsWith("2026-11-09-NOC-76-"),
  );
  const entry = files
    .map((file) => read(join(DECISIONS, file)))
    .find((text) => /^- Topics: .*\bgroups\b/m.test(text));
  assert.ok(entry, "no NOC-76 decision with the topic groups");
  assert.match(entry, /^- Decided by: .*Carsten.*2026-11-09/m);
  assert.match(
    entry,
    /^- Supersedes: .*2026-11-07-NOC-23-rfc-0008-defers-groups-as-members\.md/m,
  );
});

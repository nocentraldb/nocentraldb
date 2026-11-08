// Asserts the RFCs under docs/rfc/ that the gate can check although they are
// prose (NOC-6): RFC 0002, the change and the log format, exists, and it links
// back to RFC 0001, the design it builds on, with a relative link that
// resolves to the file, and to a heading when the link names one.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

const RFC_0001 = "docs/rfc/0001-nocentraldb.md";
const RFC_0002 = "docs/rfc/0002-change-and-log-format.md";

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

/** The relative links of a Markdown file outside code blocks, resolved. */
const relativeLinks = (path) =>
  [
    ...read(path)
      .replace(/```[\s\S]*?```/g, "")
      .matchAll(/\]\(([^)\s]+)\)/g),
  ]
    .map(([, target]) => target)
    .filter((target) => !/^[a-z]+:/.test(target))
    .map((target) => {
      const [file, anchor] = target.split("#");
      const resolved = file ? normalize(join(dirname(path), file)) : path;
      return { target, resolved, anchor };
    });

test("RFC 0002 exists", () => {
  assert.ok(existsSync(join(root, RFC_0002)), `${RFC_0002} is missing`);
});

test("RFC 0002 links back to RFC 0001, and the links resolve", () => {
  const back = relativeLinks(RFC_0002).filter((l) => l.resolved === RFC_0001);
  assert.ok(back.length > 0, `${RFC_0002} has no link to ${RFC_0001}`);
  for (const { target, anchor } of back) {
    if (!anchor) continue;
    assert.ok(
      anchors(read(RFC_0001)).has(anchor),
      `${RFC_0002}: ${target} names no heading in ${RFC_0001}`,
    );
  }
});

// NOC-22: the errata to RFC 0002. The amendment is announced by a dated
// changelog line above the first section, lives in its own section, names
// options and a recommendation per decision, and the member log knows the
// "rejected" state next to "folded" and "parked".
const AMENDMENT = "amendment-1-atomic-unit-and-rejected-changes";

/** The text of the level-2 section whose anchor is `anchor`, without the heading. */
const section = (markdown, anchor) => {
  const parts = markdown.split(/^(?=## )/m);
  const found = parts.find((part) =>
    part.startsWith("## ")
      ? slug(part.split("\n")[0].slice(3)) === anchor
      : false,
  );
  return found?.split("\n").slice(1).join("\n");
};

test("RFC 0002 announces its amendment in a dated changelog line at the top", () => {
  const top = read(RFC_0002).split(/^## /m)[0];
  assert.match(
    top,
    new RegExp(`^- \\*\\*\\d{4}-\\d{2}-\\d{2}\\*\\*.*\\(#${AMENDMENT}\\)`, "m"),
    "no dated changelog line linking to the amendment above the first section",
  );
});

test("RFC 0002 has the amendment section, and links inside the RFC resolve", () => {
  const rfc = read(RFC_0002);
  assert.ok(anchors(rfc).has(AMENDMENT), `no section #${AMENDMENT}`);
  for (const { target, anchor } of relativeLinks(RFC_0002)) {
    if (target.startsWith("#")) {
      assert.ok(
        anchors(rfc).has(anchor),
        `${RFC_0002}: ${target} names no heading`,
      );
    }
  }
});

test("each decision of the amendment has options and one recommendation", () => {
  const body = section(read(RFC_0002), AMENDMENT);
  assert.ok(body, `no section #${AMENDMENT}`);
  for (const id of ["D10", "D11", "D12"]) {
    const block = body.split(`**${id}.`)[1]?.split(/\*\*D\d+\./)[0];
    assert.ok(block, `the amendment has no decision ${id}`);
    assert.match(block, /^- A\. /m, `${id} lists no options`);
    assert.match(block, /^- B\. /m, `${id} lists fewer than two options`);
    assert.equal(
      block.match(/\*\(recommended\)\*/g)?.length,
      1,
      `${id} needs exactly one recommended option`,
    );
  }
});

test("the member log knows folded, parked and rejected", () => {
  const log = read(RFC_0002).match(/```text\nlog\n[\s\S]*?```/)?.[0];
  assert.ok(log, "no log table in RFC 0002");
  assert.match(log, /state\s+folded, parked or rejected/);
});

// NOC-38: Amendment 2 gives a payload that decrypts but does not decode its
// own row in the step table of Amendment 1, between decryption (step 5) and
// understanding the payload (step 6), and puts the choice of its state to the
// owner as D13, with the three options the ticket names.
const AMENDMENT_2 = "amendment-2-a-payload-that-decrypts-but-does-not-decode";

/** The rows of the step table of Amendment 1, as arrays of trimmed cells. */
const stepRows = (markdown) =>
  section(markdown, AMENDMENT)
    ?.split("\n")
    .filter((line) => /^\| \d+[a-z]?\. /.test(line))
    .map((line) =>
      line
        .slice(1, -1)
        .split(" | ")
        .map((cell) => cell.trim()),
    ) ?? [];

test("RFC 0002 announces Amendment 2 in a dated changelog line at the top", () => {
  const top = read(RFC_0002).split(/^## /m)[0];
  assert.match(
    top,
    new RegExp(
      `^- \\*\\*\\d{4}-\\d{2}-\\d{2}\\*\\* · NOC-38 · .*\\(#${AMENDMENT_2}\\)`,
      "m",
    ),
    "no dated changelog line for NOC-38 linking to Amendment 2 above the first section",
  );
});

test("RFC 0002 has the Amendment 2 section", () => {
  assert.ok(
    anchors(read(RFC_0002)).has(AMENDMENT_2),
    `no section #${AMENDMENT_2}`,
  );
});

test("the step table has a row for a payload that decrypts but does not decode", () => {
  const rows = stepRows(read(RFC_0002));
  const steps = rows.map(([step]) => step.split(".")[0]);
  assert.deepEqual(steps, ["1", "2", "3", "4", "5", "5a", "6", "7"]);
  const [step, fails, state] = rows[steps.indexOf("5a")];
  assert.match(step, /^5a\. Decode the payload/);
  assert.match(fails, /decrypt/i, "the row does not say the payload decrypted");
  assert.match(state, /Parked: unknown payload format/);
  assert.match(state, /Amendment 2/, "the row is not marked as Amendment 2");
});

test("D13 lists the three options of the ticket and one recommendation", () => {
  const body = section(read(RFC_0002), AMENDMENT_2);
  assert.ok(body, `no section #${AMENDMENT_2}`);
  const block = body.split("**D13.")[1]?.split(/\*\*D\d+\./)[0];
  assert.ok(block, "Amendment 2 has no decision D13");
  const options = block.match(/^- [A-Z]\. .*$/gm) ?? [];
  assert.equal(options.length, 3, "D13 needs exactly three options");
  assert.match(
    options[0],
    /^- A\. Parked.*unknown payload format.*\*\(recommended\)\*/,
  );
  assert.match(options[1], /^- B\. Rejected/);
  assert.match(options[2], /^- C\. Split by cause/);
  assert.equal(block.match(/\*\(recommended\)\*/g)?.length, 1);
});

test("Amendment 2 says how each option converges and what an upgrade does", () => {
  const body = section(read(RFC_0002), AMENDMENT_2) ?? "";
  const headings = body.match(/^### .*$/gm) ?? [];
  assert.ok(
    headings.some((h) => /converge/i.test(h)),
    "no section on convergence",
  );
  assert.ok(
    headings.some((h) => /upgrade/i.test(h)),
    "no section on upgrades",
  );
});

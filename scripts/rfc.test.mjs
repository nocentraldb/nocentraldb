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

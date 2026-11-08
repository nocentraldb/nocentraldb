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

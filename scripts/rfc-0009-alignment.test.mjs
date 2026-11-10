// Asserts that the texts RFC 0009's "Conflicts" section names follow it
// (NOC-75): each point is carried out in the text it names (the RFCs state
// them in their text since NOC-100 and NOC-101, which their own tests hold);
// the use cases use only calls RFC 0009 defines; and RFC 0009 marks each point resolved. The test
// checks that each change is there and points to its source; whether the
// wording is right is the owner's review of the PR.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

const RFC_0001 = "docs/rfc/0001-nocentraldb.md";
const RFC_0003 = "docs/rfc/0003-keys-and-cryptography.md";
const RFC_0005 = "docs/rfc/0005-schema-and-model-lock.md";
const RFC_0006 = "docs/rfc/0006-local-view.md";
const RFC_0008 = "docs/rfc/0008-membership.md";
const RFC_0009 = "docs/rfc/0009-sdk-api.md";
const USE_CASES = "docs/use-cases.md";

const NOT_COVERED = "0009-sdk-api.md#what-this-rfc-does-not-cover";

/**
 * Each amended RFC, the number of its amendment, and the RFC 0009 points it
 * carries out. RFC 0005 (points 1, 3 and 4), RFC 0006 (points 1 and 2) and
 * RFC 0008 (point 10) carry them in their text since NOC-101 folded their
 * amendments in; the tests below check them where they stand.
 */

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

/** The code blocks of a Markdown text, joined. */
const code = (markdown) =>
  [...markdown.matchAll(/```[a-z]*\n([\s\S]*?)```/g)]
    .map((m) => m[1])
    .join("\n");

/** The relative links of a Markdown file outside code blocks, resolved. */
const relativeLinks = (path) =>
  targets(read(path).replace(/```[\s\S]*?```/g, ""))
    .filter((target) => !/^[a-z]+:/.test(target))
    .map((target) => {
      const [file, anchor] = target.split("#");
      const resolved = file ? normalize(join(dirname(path), file)) : path;
      return { target, resolved, anchor };
    });

test("RFC 0003's sketch uses RFC 0009's names: no name for a space, the role writer, and a lock", () => {
  const sketch = code(section(read(RFC_0003), "how-the-sdk-hides-keys"));
  assert.ok(!/"editor"/.test(sketch), "the sketch still uses the role editor");
  assert.ok(
    !/spaces\.create\(\{/.test(sketch),
    "the sketch still names a space",
  );
  assert.match(sketch, /spaces\.create\(\)/);
  assert.match(sketch, /role: "writer"/);
  assert.match(sketch, /nc\.open\(\{ model, lock \}\)/);
});

test("RFC 0003 says the source of a copy is kept on the parent only", () => {
  const step = section(read(RFC_0003), "sub-agents-get-an-ad-hoc-space")
    .split("\n")
    .find((l) => l.startsWith("3. "));
  assert.match(step, /parent/);
  assert.match(step, /source/);
  assert.match(step, /never learns/);
});

test("RFC 0005 types an unknown option as nc.Unknown and names the refusal noPerson", () => {
  const rfc = read(RFC_0005);
  assert.match(section(rfc, "types-for-the-app"), /nc\.Unknown/);
  assert.match(section(rfc, "what-the-build-checks"), /`noPerson`/);
});

test("RFC 0005 has the index option scope and its key in the lock", () => {
  const rfc = read(RFC_0005);
  assert.match(section(rfc, "indexes"), /`scope: "space"`/);
  assert.match(
    rfc.split("\n").find((l) => l.startsWith("| Index keys")) ?? "",
    /`scope`/,
    "the lock's index keys do not list scope",
  );
  assert.match(
    rfc.split("\n").find((l) => l.startsWith("| `nc.index`")) ?? "",
    /`scope`/,
    "the public name nc.index does not list the option scope",
  );
});

test("RFC 0006 puts the space id first in an index with scope space, and keeps deleted objects out of every index", () => {
  const text = section(read(RFC_0006), "declared-indexes");
  assert.match(text, /`scope: "space"`/, "no index with scope space");
  assert.match(
    text,
    /\(space_id, [a-z_, ]*object_id\)/,
    "no column order with the space id first",
  );
  assert.match(text, /deleted object/i, "says nothing about deleted objects");
});

test("RFC 0001's example queries a declared index, not a view", () => {
  const example = code(section(read(RFC_0001), "the-design-in-one-page"));
  assert.ok(!/views\.ready/.test(example), "the example still queries a view");
  assert.match(example, /\.indexes\.\w+\.equal\(/);
});

test("RFC 0008 links RFC 0009 for invitations and public names", () => {
  const bullet = section(read(RFC_0008), "what-this-rfc-does-not-cover")
    .split("\n")
    .find((l) => l.startsWith("- **Invitations and public API names"));
  assert.ok(bullet, "no bullet on invitations");
  assert.ok(links(bullet, "0009-sdk-api.md#invitations-and-members"));
});

test("the use cases use only calls RFC 0009 defines, and link what it does not cover", () => {
  const text = read(USE_CASES);
  const calls = code(text);
  for (const old of [
    /"editor"/,
    /\bdb\.get\(/,
    /\bdb\.insert\(/,
    /nc\.useLive/,
    /\bwhere: /,
    /views\.ready/,
    /\.personal\(\)|personal: true/,
  ]) {
    assert.ok(!old.test(calls), `the use cases still use ${old}`);
  }
  assert.ok(
    links(text, NOT_COVERED),
    "no link to RFC 0009's What this RFC does not cover",
  );
});

test("every relative link in the amended texts resolves, anchors included", () => {
  for (const path of [
    RFC_0001,
    RFC_0003,
    RFC_0005,
    RFC_0006,
    RFC_0008,
    RFC_0009,
    USE_CASES,
  ]) {
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

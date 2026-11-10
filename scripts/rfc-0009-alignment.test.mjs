// Asserts that the texts RFC 0009's "Conflicts" section names follow it
// (NOC-75): each point is carried out in the text it names, as a marked
// amendment that decides nothing and links the point; the use cases use only
// calls RFC 0009 defines; and RFC 0009 marks each point resolved. The test
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

const CONFLICTS = "0009-sdk-api.md#conflicts-with-genesismd-and-the-other-rfcs";

/**
 * Each amended RFC, the number of its amendment, and the RFC 0009 points it
 * carries out. RFC 0005 (points 1, 3 and 4), RFC 0006 (points 1 and 2) and
 * RFC 0008 (point 10) carry them in their text since NOC-101 folded their
 * amendments in; the tests below check them where they stand.
 */
const AMENDMENTS = [
  { path: RFC_0001, number: 6, points: [6] },
  { path: RFC_0003, number: 3, points: [5, 9] },
];
const anchorOf = (number) => `amendment-${number}-following-rfc-0009`;

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

for (const { path, number, points } of AMENDMENTS) {
  const anchor = anchorOf(number);

  test(`${path} announces Amendment ${number} in a dated NOC-75 changelog line at the top`, () => {
    const top = read(path).split(/^## /m)[0];
    assert.match(
      top,
      new RegExp(
        `^- \\*\\*\\d{4}-\\d{2}-\\d{2}\\*\\* · NOC-75 · .*\\(#${anchor}\\)`,
        "m",
      ),
      `no changelog line for NOC-75 linking to #${anchor} above the first section`,
    );
  });

  test(`${path} has Amendment ${number}, which decides nothing, and every point links RFC 0009's point`, () => {
    const body = section(read(path), anchor);
    assert.ok(body, `no section #${anchor}`);
    assert.match(
      body,
      /decides nothing/,
      "the amendment does not say it decides nothing",
    );
    const bullets = body.split("\n").filter((line) => line.startsWith("- **"));
    assert.equal(bullets.length, points.length, "one bullet per point");
    for (const point of points) {
      const bullet = bullets.find((b) =>
        new RegExp(`\\bpoint ${point}\\b`).test(b),
      );
      assert.ok(bullet, `no bullet names RFC 0009's point ${point}`);
      assert.ok(
        links(bullet, CONFLICTS),
        `point ${point} does not link RFC 0009's Conflicts`,
      );
    }
  });
}

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

test("RFC 0009's point 1 is resolved for RFC 0006 too", () => {
  const body = section(
    read(RFC_0009),
    "conflicts-with-genesismd-and-the-other-rfcs",
  );
  const line = body.split("\n").find((l) => l.startsWith("1. "));
  assert.ok(links(line, "0006-local-view.md#declared-indexes"));
  assert.ok(!/RFC 0006 follows/.test(line), "point 1 still waits for RFC 0006");
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

test("the use cases use only calls RFC 0009 defines, and link its Conflicts", () => {
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
  assert.ok(links(text, CONFLICTS), "no link to RFC 0009's Conflicts");
});

test("RFC 0009 marks each point it carried out resolved", () => {
  const body = section(
    read(RFC_0009),
    "conflicts-with-genesismd-and-the-other-rfcs",
  );
  for (const point of [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]) {
    const line = body.split("\n").find((l) => l.startsWith(`${point}. `));
    assert.ok(line, `no point ${point}`);
    assert.match(line, /\*Resolved\b/, `point ${point} is not marked resolved`);
  }
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

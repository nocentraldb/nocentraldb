// Asserts RFC 0005 as one complete text (NOC-101): every amendment is folded
// into the design where its rule belongs, so no "Amendment" section, mark or
// in-page link is left; "Decisions" keeps each decision in short form, with
// the owner's name and date, why, and the options not taken; "History" lists
// dated lines that name their ticket and pull request; and every link other
// files use into RFC 0005 still resolves. The rules the amendments made are
// checked where they now stand: the writer's model in payload key 64
// (NOC-54), RFC 0001's wording on personal fields (NOC-35), the index option
// `scope`, `nc.Unknown` and `noPerson` (NOC-75), the lock example's key order,
// `nc.int` as a `number` and deprecated collections (NOC-73), and `nc.ref`
// (NOC-63).
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, normalize, relative } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0005-schema-and-model-lock.md";
const FIXTURE = "crates/ncdb-core/tests/model_lock/rfc-0005-v2.lock.json";

const read = (path = RFC) => readFileSync(join(root, path), "utf8");

/** GitHub's heading slug: lower case, punctuation dropped, spaces to dashes. */
const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N} _-]/gu, "")
    .replace(/ /g, "-");

const headings = (markdown) =>
  markdown
    .split("\n")
    .filter((l) => /^#{1,6} /.test(l))
    .map((l) => l.replace(/^#+ /, ""));

/** The text under the heading whose anchor is `anchor`, up to the next heading of its level or higher. */
const section = (markdown, anchor) => {
  const lines = markdown.split("\n");
  const start = lines.findIndex(
    (l) => /^#{1,6} /.test(l) && slug(l.replace(/^#+ /, "")) === anchor,
  );
  assert.ok(start >= 0, `${RFC} has no heading #${anchor}`);
  const level = lines[start].match(/^#+/)[0].length;
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((l) => {
    const m = l.match(/^(#+) /);
    return m && m[1].length <= level;
  });
  return (end < 0 ? rest : rest.slice(0, end)).join("\n");
};

/** The first line of `markdown` that starts with `start`. */
const line = (markdown, start) =>
  markdown.split("\n").find((l) => l.startsWith(start)) ?? "";

const jsonBlocks = (markdown) =>
  [...markdown.matchAll(/^```json\n([\s\S]*?)^```$/gm)].map((m) => m[1]);

/** The decisions under "## Decisions", one "### Dn. Title" heading each. */
const decisions = () => {
  const body = section(read(), "decisions");
  return [
    ...body.matchAll(/^### (D\d+)\. [^\n]+\n([\s\S]*?)(?=^### |(?![\s\S]))/gm),
  ].map((m) => ({ id: m[1], text: m[2] }));
};
const decision = (id) => {
  const found = decisions().find((d) => d.id === id);
  assert.ok(found, `${RFC} has no decision ${id}`);
  return found.text;
};

test("RFC 0005 has no amendment section, mark, in-page link or changelog left", () => {
  const rfc = read();
  for (const heading of headings(rfc)) {
    assert.doesNotMatch(heading, /amendment/i, `heading "${heading}"`);
  }
  assert.doesNotMatch(rfc, /\*\(?Amendment \d+\.?\)?\*/, "a mark is left");
  assert.doesNotMatch(
    rfc,
    /\]\(#amendment-/,
    "an in-page link to an amendment",
  );
  assert.doesNotMatch(rfc, /^\*\*Changelog\.\*\*/m, "the changelog is left");
  const status = line(rfc, "**Status.**");
  assert.doesNotMatch(status, /Amendment/, "the status names an amendment");
});

test("the status names every date the owner decided", () => {
  const status = line(read(), "**Status.**");
  assert.match(status, /^\*\*Status\.\*\* Decided\. /);
  for (const date of ["2026-11-08", "2026-11-09", "2026-11-10"]) {
    assert.ok(status.includes(`(Carsten, ${date})`), `no decision of ${date}`);
  }
});

test("History lists dated lines, each with its ticket and pull request", () => {
  const history = section(read(), "history");
  const entries = history.split("\n").filter((l) => l.startsWith("- "));
  for (const [ticket, pr] of [
    ["NOC-11", 35],
    ["NOC-54", 70],
    ["NOC-35", 76],
    ["NOC-75", 87],
    ["NOC-73", 88],
    ["NOC-63", 92],
    ["NOC-101", null],
  ]) {
    const entry = entries.find((l) => l.includes(` · ${ticket} · `));
    assert.ok(entry, `History has no line for ${ticket}`);
    assert.match(entry, /^- \*\*\d{4}-\d{2}-\d{2}\*\* · NOC-\d+ · /);
    if (pr) assert.ok(entry.includes(`(#${pr})`), `${ticket} names no #${pr}`);
    else assert.match(entry, /\(#\d+\)/, `${ticket} names no pull request`);
  }
});

test("Open questions says whether the owner still has one", () => {
  assert.match(section(read(), "open-questions"), /\S/);
});

test("Decisions keeps D1 to D22 in short form: decided, why, and the options not taken", () => {
  const found = decisions();
  assert.deepEqual(
    found.map((d) => d.id),
    Array.from({ length: 22 }, (_, i) => `D${i + 1}`),
  );
  const dates = { D16: "2026-11-09", D17: "2026-11-09" };
  for (const { id, text } of found) {
    const n = Number(id.slice(1));
    const date = dates[id] ?? (n >= 18 ? "2026-11-10" : "2026-11-08");
    assert.match(
      text,
      new RegExp(`^Decided \\(Carsten, ${date}\\): `, "m"),
      id,
    );
    assert.match(text, /^Why: /m, `${id} gives no reason`);
    assert.match(
      text,
      /^Not taken:\n\n?- /m,
      `${id} lists no option not taken`,
    );
    assert.doesNotMatch(text, /\*\(recommended\)\*|^Recommendation:/m, id);
  }
});

test("the writer's model in payload key 64 stands in the design and in D10 (NOC-54)", () => {
  const rfc = read();
  assert.doesNotMatch(rfc, /Nothing in a change says which model wrote it/);
  assert.doesNotMatch(rfc, /falls back to B/);
  const newer = section(rfc, "data-from-a-newer-model");
  const point = line(newer, "6. ");
  assert.match(point, /another model/);
  assert.match(point, /apart/);
  assert.match(
    newer.split("\n").find((l) => l.startsWith("- ") && /model\.id/.test(l)) ??
      "",
    /payload key 64/,
  );
  assert.match(decision("D10"), /payload key 64/);
  assert.match(decision("D13"), /payload key 64/);
});

test("Personal fields quotes RFC 0001's wording: basic values only (NOC-35)", () => {
  const personal = section(read(), "personal-fields");
  assert.doesNotMatch(personal, /says any field can be marked as personal/);
  assert.match(
    personal.split("\n").find((l) => l.includes("0001-nocentraldb.md")) ?? "",
    /basic value/,
  );
});

test("the index option scope, nc.Unknown and noPerson stand in the design (NOC-75)", () => {
  const rfc = read();
  assert.match(section(rfc, "indexes"), /`scope: "space"`/);
  assert.match(line(rfc, "Per index:"), /`scope`/);
  assert.match(section(rfc, "types-for-the-app"), /nc\.Unknown/);
  assert.match(section(rfc, "what-the-build-checks"), /`noPerson`/);
  assert.match(line(rfc, "| Add, change or remove an index"), /scope/);
});

/** RFC 8785 sorts keys by their UTF-16 code units, as `<` compares strings. */
const sorted = (keys) => [...keys].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
const keyOrders = (value, path = "$") => {
  if (Array.isArray(value)) {
    return value.flatMap((v, i) => keyOrders(v, `${path}[${i}]`));
  }
  if (value === null || typeof value !== "object") return [];
  const keys = Object.keys(value);
  return [
    { path, keys },
    ...keys.flatMap((k) => keyOrders(value[k], `${path}.${k}`)),
  ];
};

test("the lock example is canonical and the bytes ncdb model build writes (NOC-73)", () => {
  const example = jsonBlocks(section(read(), "an-example"))[0];
  assert.equal(example, read(FIXTURE));
  for (const { path, keys } of keyOrders(JSON.parse(example))) {
    assert.deepEqual(keys, sorted(keys), `keys of ${path} not sorted`);
  }
});

test("nc.int reads as a number, and a collection can be deprecated (NOC-73, D16, D17)", () => {
  const rfc = read();
  const types = section(rfc, "types-for-the-app");
  assert.match(types, /`nc\.int` field as a `number`/);
  assert.match(types, /safe integer/);
  assert.match(types, /`\$raw`/);
  assert.match(line(rfc, "- `.deprecated(reason)`"), /nc\.collection\(/);
  assert.match(line(rfc, "Per collection:"), /`deprecatedReason`/);
  assert.match(line(rfc, "| `.deprecated(`"), /collection/);
  assert.match(line(rfc, "| Collection keys "), /`deprecatedReason`/);
  assert.match(decision("D16"), /`number`/);
  assert.match(decision("D16"), /`bigint`/);
  assert.match(decision("D17"), /\.deprecated\(/);
});

test("nc.ref stands in the schema, the lock, the additive-change rule and Public names (NOC-63)", () => {
  const rfc = read();
  assert.match(line(rfc, '| `nc.ref("collection")` |'), /`ncdb\/ref`/);
  assert.match(line(rfc, "- **Built-in types**"), /`ncdb\/ref`/);
  assert.match(line(rfc, "| `targets` |"), /`ncdb\/ref`/);
  const refs = section(rfc, "references-into-another-space");
  assert.match(refs, /another space/);
  assert.match(refs, /never follow/);
  assert.match(refs, /nc\.ref\("customers"\)/);
  assert.match(refs, /"type": "ncdb\/ref"/);
  assert.match(refs, /subscri/, "it does not say what a subscription follows");
  assert.match(
    refs,
    /\.personal\(/,
    "it does not say a reference is not personal",
  );
  assert.match(line(rfc, "- An index can use"), /`nc\.ref`/);
  assert.match(section(rfc, "types-for-the-app"), /\{ space, id \}/);
  assert.match(
    line(rfc, "| Add a collection to a link's or a reference's `targets`"),
    /missing/,
  );
  assert.ok(
    line(rfc, "| Remove a collection from a link's or a reference's `targets`"),
  );
  const names = section(rfc, "public-names");
  assert.match(line(names, "| `nc.ref` |"), /another space/);
  assert.match(line(names, "| The type id namespace"), /`ncdb\/ref`/);
  assert.match(decision("D18"), /`targets`/);
  assert.match(decision("D19"), /index/);
  assert.match(decision("D20"), /`"notAccessible"`/);
  assert.match(decision("D21"), /subscription/);
  assert.match(decision("D22"), /\.personal\(/);
});

test("every in-page link of RFC 0005 resolves to a heading", () => {
  const rfc = read();
  const anchors = new Set(headings(rfc).map(slug));
  for (const [, anchor] of rfc.matchAll(/\]\(#([^)\s]+)\)/g)) {
    assert.ok(anchors.has(anchor), `${RFC} links to missing #${anchor}`);
  }
});

/** Every Markdown and test file of the repository outside dependencies and builds. */
const files = (dir = root) =>
  readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    if (["node_modules", "target", ".git", "dist"].includes(e.name)) return [];
    const path = join(dir, e.name);
    if (e.isDirectory()) return files(path);
    return /\.(md|mjs)$/.test(e.name) ? [relative(root, path)] : [];
  });

test("every link other files use into RFC 0005 names one of its headings", () => {
  const anchors = new Set(headings(read()).map(slug));
  for (const path of files()) {
    if (normalize(path) === normalize(RFC)) continue;
    for (const [, anchor] of read(path).matchAll(
      /0005-schema-and-model-lock\.md#([a-z0-9-]+)/g,
    )) {
      assert.ok(anchors.has(anchor), `${path} links to missing #${anchor}`);
    }
  }
});

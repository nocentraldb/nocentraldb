// NOC-62: RFC 0002 Amendment 6 records two rules the owner decided on
// 2026-11-09: a change nests at most 32 levels, checked by every member and
// by the relay before it recurses (NOC-24), and a clock whose counter is full
// carries into the next millisecond with counter 0 (NOC-25). The test checks
// the changelog line and its link, that both rules are marked where they
// change the RFC and name the owner, the date and the ticket, and that the
// byte-exact examples N1 (32 levels, accepted) and N2 (33 levels, refused)
// behave so under this reader and equal the committed vectors that
// docs/rfc/vectors/0002/generate.py writes with Python's cbor2.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import {
  decode,
  encode,
  fromHex,
  MAX_DEPTH,
  ProfileError,
  toHex,
} from "./lib/cbor-profile.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0002-change-and-log-format.md";
const VECTORS = "docs/rfc/vectors/0002";
const AMENDMENT = "amendment-6-a-nesting-limit-and-a-clock-that-never-waits";
const MARK = /\*\(Amendment 6\.\)\*/;
const read = (path) => readFileSync(join(root, path), "utf8");

/** The payload map sits at depth 3: frame 1, header 2, payload 3. */
const PAYLOAD_DEPTH = 3;

/** GitHub's heading anchor: lower case, punctuation dropped, spaces to dashes. */
const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .replace(/\s/g, "-");

/** The text of the section at `level` whose anchor is `anchor`, without the heading. */
const section = (markdown, anchor, level = 2) => {
  const hashes = "#".repeat(level);
  return markdown
    .split(new RegExp(`^(?=${hashes} )`, "m"))
    .find((part) => slug(part.split("\n")[0].slice(level + 1)) === anchor)
    ?.split("\n")
    .slice(1)
    .join("\n")
    .split(new RegExp(`^#{1,${level}} `, "m"))[0];
};

test("RFC 0002 announces Amendment 6 in a dated changelog line and its status", () => {
  const top = read(RFC).split(/^## /m)[0];
  assert.match(
    top,
    new RegExp(
      `^- \\*\\*2026-11-09\\*\\* · NOC-62 · .*\\(#${AMENDMENT}\\)`,
      "m",
    ),
    "no dated changelog line for NOC-62 linking to Amendment 6",
  );
  const status = top.split("**Status.**")[1] ?? "";
  assert.match(
    status,
    new RegExp(`\\(#${AMENDMENT}\\)`),
    "the status omits Amendment 6",
  );
});

test("the amendment says the owner decided both rules, with the date and the tickets", () => {
  const body = section(read(RFC), AMENDMENT);
  assert.ok(body, `no section #${AMENDMENT}`);
  assert.match(body, /Carsten, 2026-11-09/);
  assert.match(body, /\bNOC-24\b/);
  assert.match(body, /\bNOC-25\b/);
});

test("the CBOR profile limits nesting to 32 levels, byte strings not counted, marked", () => {
  const profile = section(read(RFC), "the-cbor-profile", 3) ?? "";
  const rule = profile
    .split("\n")
    .find((line) => line.startsWith("- **Nesting"));
  assert.ok(rule, "the profile has no nesting rule");
  assert.match(rule, /\b32 levels\b/);
  assert.match(
    rule,
    /byte string/i,
    "the rule does not say byte strings do not count",
  );
  assert.match(
    rule,
    /before it recurses/,
    "the rule does not say when it is checked",
  );
  assert.match(rule, /NOC-24/);
  assert.match(rule, MARK, "the nesting rule is not marked");
  assert.equal(
    MAX_DEPTH,
    32,
    "the tooling reader's limit differs from the RFC",
  );
});

test("the relay checks the nesting limit before it accepts a change, marked", () => {
  const relay = section(read(RFC), "on-the-relay", 3) ?? "";
  const accepts = relay
    .split("\n")
    .find((line) => line.startsWith("The relay accepts a change when"));
  assert.match(accepts ?? "", /32 levels/, "the relay's check omits the limit");
  assert.match(accepts ?? "", MARK, "the relay's check is not marked");
});

test("a full counter carries into the next millisecond, and the member never waits", () => {
  const clock = section(read(RFC), "the-clock", 3) ?? "";
  const rule = clock.split("\n").find((line) => /counter is full/.test(line));
  assert.ok(rule, '"The clock" has no rule for a full counter');
  assert.match(rule, /next millisecond with counter 0/);
  assert.match(rule, /never waits/);
  assert.match(rule, /NOC-25/);
  assert.match(rule, MARK, "the clock rule is not marked");
  assert.doesNotMatch(
    clock,
    /waits for its wall clock/,
    '"The clock" still has the member wait for its wall clock',
  );
});

test("no sentence still calls either rule pending the owner", () => {
  const rfc = read(RFC);
  assert.doesNotMatch(rfc, /pending the owner in NOC-24/);
  assert.doesNotMatch(rfc, /pending the owner in NOC-25/);
});

/** The examples of Amendment 6, by name: { N1: "a5…", … }. */
const examples = () => {
  const body = section(read(RFC), AMENDMENT) ?? "";
  const part = body.split(/^### Byte-exact examples$/m)[1] ?? "";
  const found = {};
  for (const example of part.split(/^\*\*(?=N\d+\.)/m).slice(1)) {
    const name = example.match(/^(N\d+)\./)[1];
    const hex = [...example.matchAll(/```text\n((?:[0-9a-f]+\n)+)```/g)];
    assert.equal(hex.length, 1, `${name} has ${hex.length} hex blocks, not 1`);
    found[name] = hex[0][1].replace(/\s+/g, "");
  }
  return found;
};

/** How deep the deepest item of a decoded value sits, when the value is at `depth`. */
const deepest = (value, depth) => {
  if (Array.isArray(value)) {
    return Math.max(depth, ...value.map((item) => deepest(item, depth + 1)));
  }
  if (value && Array.isArray(value.map)) {
    return Math.max(
      depth,
      ...value.map.map(([, item]) => deepest(item, depth + 1)),
    );
  }
  return depth;
};

test("N1 nests 32 levels and decodes; N2 nests 33 and is refused", () => {
  const found = examples();
  assert.deepEqual(Object.keys(found), ["N1", "N2"]);
  const n1 = decode(fromHex(found.N1), PAYLOAD_DEPTH);
  assert.equal(toHex(encode(n1)), found.N1);
  assert.equal(deepest(n1, PAYLOAD_DEPTH), 32, "N1 does not reach level 32");
  assert.throws(
    () => decode(fromHex(found.N2), PAYLOAD_DEPTH),
    (error) =>
      error instanceof ProfileError &&
      /nested deeper than 32/.test(error.message),
    "N2 is not refused as too deep",
  );
  const unchecked = decode(fromHex(found.N2), PAYLOAD_DEPTH - 1);
  assert.equal(
    deepest(unchecked, PAYLOAD_DEPTH),
    33,
    "N2 does not reach level 33",
  );
  assert.equal(
    found.N2.length,
    found.N1.length + 2,
    "N2 is not N1 one level deeper",
  );
});

test("N1 and N2 equal the committed vectors of the generator", () => {
  for (const [name, hex] of Object.entries(examples())) {
    const vector = read(join(VECTORS, `${name}.hex`)).replace(/\s+/g, "");
    assert.equal(hex, vector, `${name} differs from ${VECTORS}/${name}.hex`);
  }
});

test("the core cites Amendment 6 for both rules, and calls neither pending", () => {
  const files = {
    "crates/ncdb-core/src/cbor.rs": /Amendment 6/,
    "crates/ncdb-core/src/clock.rs": /Amendment 6/,
    "crates/ncdb-core/tests/depth.rs": /Amendment 6/,
    "crates/ncdb-core/tests/clock.rs": /Amendment 6/,
  };
  for (const [path, cites] of Object.entries(files)) {
    const text = read(path);
    assert.match(text, cites, `${path} does not cite Amendment 6`);
    assert.doesNotMatch(
      text.replace(/\s*\/\/[!/]?\s*/g, " "),
      /pending the owner/,
      `${path} still calls a rule pending the owner`,
    );
  }
});

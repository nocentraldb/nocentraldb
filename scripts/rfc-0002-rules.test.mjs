// Holds the rules of RFC 0002 that its six amendments brought in, checked
// where they stand now that RFC 0002 is one complete text (NOC-100):
// what a member does with a change it receives (NOC-22, NOC-38), what RFC
// 0008 and RFC 0007 changed (NOC-44, NOC-47), the writer's model in payload
// key 64 (NOC-40), and the nesting limit and the clock's carry (NOC-62). The
// byte-exact examples M1, M2, N1 and N2 decode under RFC 0002's profile and
// equal the committed vectors that docs/rfc/vectors/0002/generate.py writes
// with Python's cbor2, independently of this reader and of the Rust core.
// rfc-0002-complete.test.mjs holds the text's form as a fresh text (NOC-165):
// no list of decisions, each decision in its section, one History line.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
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
const read = (path) => readFileSync(join(root, path), "utf8");

const RFC = "docs/rfc/0002-change-and-log-format.md";
const VECTORS = "docs/rfc/vectors/0002";

/** The payload map sits at depth 3 (frame, header, payload); key 64's value at 4. */
const PAYLOAD_DEPTH = 3;
const MODEL_DEPTH = 4;

/** GitHub's heading anchor: lower case, punctuation dropped, spaces to dashes. */
const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .replace(/\s/g, "-");

/** The text under the heading whose anchor is `anchor`, up to the next heading of the same or a higher level. */
const section = (anchor) => {
  const lines = read(RFC).split("\n");
  const start = lines.findIndex(
    (line) =>
      /^#{1,6} /.test(line) && slug(line.replace(/^#+ /, "")) === anchor,
  );
  assert.ok(start >= 0, `RFC 0002 has no section #${anchor}`);
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

/** The first line of `text` that starts with `start`. */
const line = (text, start) => {
  const found = text.split("\n").find((l) => l.startsWith(start));
  assert.ok(found, `no line starts with "${start}"`);
  return found;
};

/** The rows of the step table, as arrays of trimmed cells. */
const stepRows = () =>
  section("what-a-member-does-with-a-change-it-receives")
    .split("\n")
    .filter((l) => /^\| \d+[a-z]?\. /.test(l))
    .map((l) =>
      l
        .slice(1, -1)
        .split(" | ")
        .map((cell) => cell.trim()),
    );

test("the step table has steps 1 to 7 and 5a, with the states of step 5a and key 64", () => {
  const rows = stepRows();
  const steps = rows.map(([step]) => step.split(".")[0]);
  assert.deepEqual(steps, ["1", "2", "3", "4", "5", "5a", "6", "7"]);
  const [step5a, fails5a, state5a] = rows[steps.indexOf("5a")];
  assert.match(step5a, /^5a\. Decode the payload/);
  assert.match(fails5a, /decrypt/i);
  assert.match(state5a, /Parked: unknown payload format/);
  const [, fails6, state6] = rows[steps.indexOf("6")];
  assert.match(fails6, /model\.id/);
  assert.match(state6, /Parked: .*another model/);
  assert.ok(links(fails6, "#the-writers-model"));
  const [, fails4] = rows[steps.indexOf("4")];
  assert.ok(links(fails4, "0008-membership.md#sealed-changes"));
});

test("the atomic unit: atomic for every check, an unknown operation parked on its own", () => {
  const text = section("the-atomic-unit");
  assert.match(text, /accepts all of its operations or none/);
  assert.match(text, /parked on its own/);
  assert.match(text, /\*\*What this does not do\.\*\*/);
  assert.ok(
    links(
      line(read(RFC), "A change carries one or more operations"),
      "#the-atomic-unit",
    ),
    "Operations does not point to The atomic unit",
  );
});

test("a rejected change is checked again when a control change arrives, and never dropped", () => {
  const text = section("rejected-changes");
  assert.match(text, /failed step 3, 4 or 5/);
  assert.match(text, /control change/);
  assert.match(text, /never dropped/);
  assert.ok(links(text, "0008-membership.md#what-a-new-change-can-change"));
});

test("a change the relay refuses stays folded, and is pushed again once the fold newly accepts it", () => {
  const text = section("a-change-the-relay-refuses-after-the-member-folded-it");
  assert.match(text, /keeps the change folded/);
  assert.match(text, /relay refusal/);
  assert.match(text, /newly accepts/);
  assert.match(text, /pushes it to that relay once more/);
});

test("a payload that decrypts but does not decode is parked, whatever the cause", () => {
  const text = section("a-payload-that-decrypts-but-does-not-decode");
  for (const cause of [
    /Too deep/,
    /Not canonical/,
    /Wrong shape/,
    /newer payload format/,
  ]) {
    assert.match(text, cause, `the cause ${cause} is not named`);
  }
  assert.match(text, /"unknown payload format"/);
  assert.match(text, /upgrade/i, "no word on what an upgrade does");
  assert.match(text, /never repaired/);
});

test("a change that nests too deep: not appended when the frame or header does, parked when the payload does", () => {
  const text = section("a-change-that-nests-too-deep");
  assert.match(text, /not appended/);
  assert.match(text, /step 5a/);
  assert.match(text, /"unknown payload format"/);
});

test("the CBOR profile limits nesting to 32 levels, byte strings counted as one item", () => {
  const rule = line(section("the-cbor-profile"), "- **Nesting");
  assert.match(rule, /\b32 levels\b/);
  assert.match(rule, /byte string/i);
  assert.match(rule, /before it recurses/);
  assert.equal(
    MAX_DEPTH,
    32,
    "the tooling reader's limit differs from the RFC",
  );
});

test("a full counter carries into the next millisecond, and the member never waits", () => {
  const clock = section("the-clock");
  const rule = clock.split("\n").find((l) => /counter is full/.test(l));
  assert.ok(rule, '"The clock" has no rule for a full counter');
  assert.match(rule, /next millisecond with counter 0/);
  assert.match(rule, /never waits/);
  assert.doesNotMatch(clock, /waits for its wall clock/);
});

test("the relay checks the nesting limit and an authenticated uploader, and applies RFC 0008's cutoff", () => {
  const accepts = line(
    section("on-the-relay"),
    "The relay accepts a change when",
  );
  assert.match(accepts, /32 levels/);
  assert.match(accepts, /authenticated current member/);
  assert.ok(links(accepts, "0007-relay.md#what-the-relay-checks"));
  assert.match(accepts, /cutoff/);
  assert.ok(
    links(accepts, "0008-membership.md#how-the-relay-applies-the-rules"),
  );
});

test("what the relay sees links RFC 0007's list, and names the control changes and key 64", () => {
  const text = section("what-the-relay-can-see-and-what-it-cannot");
  assert.ok(links(text, "0007-relay.md#what-the-relay-sees"));
  const control = line(text, "- **What the control changes say**");
  assert.match(control, /cutoff/);
  assert.match(control, /account key/);
  const model = line(text, "- **The model**");
  assert.match(model, /key 64/);
});

test("RFC 0008's format rules stand in the parts of a change", () => {
  assert.ok(links(section("space"), "0008-membership.md#what-it-names"));
  const generation = section("key-generation-suite-and-kind");
  assert.match(generation, /ordinal/);
  assert.ok(links(generation, "0008-membership.md#generation-ids"));
  assert.match(generation, /key 2, the clock, is absent/);
  const author = section("author-and-author-sequence");
  assert.match(author, /lower change id is the genesis/);
  assert.match(author, /cutoff/);
});

test("the writer's model: key 64 in the may-ignore range, a change of another model parked whole", () => {
  const text = section("the-writers-model");
  assert.match(text, /1 model id\s+16 bytes/);
  assert.match(text, /2 model version\s+uint/);
  assert.match(text, /"may ignore"/);
  assert.match(text, /parked, whole, with the reason "another model"/);
  assert.match(text, /never parks a change by itself/);
  assert.match(text, /relay sees nothing new/i);
  assert.ok(
    links(text, "0005-schema-and-model-lock.md#data-from-a-newer-model"),
  );
  const glance = section("a-change-at-a-glance");
  const payload = glance.split(/^payload \(/m)[1]?.split(/\n\n/)[0] ?? "";
  const key = payload.split("\n").find((l) => /model/i.test(l));
  assert.ok(key, "the payload tree lists no model key");
  assert.ok(
    Number(key.match(/^\S+\s+(\d+) /)?.[1]) >= 64,
    "key 64 is not may-ignore",
  );
  assert.match(
    line(
      section("versioning-old-clients-keep-what-they-do-not-understand"),
      "- **A change of another model**",
    ),
    /"another model"/,
  );
});

/** The byte-exact examples, by name: { M1: "a2…", … }. */
const examples = () => {
  const found = {};
  for (const example of section("byte-exact-examples")
    .split(/^\*\*(?=[MN]\d+\.)/m)
    .slice(1)) {
    const name = example.match(/^([MN]\d+)\./)[1];
    const hex = [...example.matchAll(/```text\n((?:[0-9a-f]+\n)+)```/g)];
    assert.equal(hex.length, 1, `${name} has ${hex.length} hex blocks, not 1`);
    found[name] = hex[0][1].replace(/\s+/g, "");
  }
  return found;
};

const entries = (map) => new Map(map.map.map(([k, v]) => [Number(k), v]));

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

test("M1 is key 64's value and M2 a payload that carries it, both canonical", () => {
  const found = examples();
  assert.deepEqual(Object.keys(found), ["M1", "M2", "N1", "N2"]);
  const m1 = decode(fromHex(found.M1), MODEL_DEPTH);
  assert.equal(toHex(encode(m1)), found.M1);
  assert.deepEqual([...entries(m1).keys()], [1, 2]);
  assert.equal(entries(m1).get(1).length, 16, "the model id is not 16 bytes");
  assert.equal(typeof entries(m1).get(2), "bigint");
  const m2 = decode(fromHex(found.M2), PAYLOAD_DEPTH);
  assert.equal(toHex(encode(m2)), found.M2);
  assert.deepEqual([...entries(m2).keys()], [1, 2, 3, 4, 64]);
  assert.equal(toHex(encode(entries(m2).get(64))), found.M1);
});

test("N1 nests 32 levels and decodes; N2 nests 33 and is refused", () => {
  const { N1, N2 } = examples();
  const n1 = decode(fromHex(N1), PAYLOAD_DEPTH);
  assert.equal(toHex(encode(n1)), N1);
  assert.equal(deepest(n1, PAYLOAD_DEPTH), 32);
  assert.throws(
    () => decode(fromHex(N2), PAYLOAD_DEPTH),
    (error) =>
      error instanceof ProfileError &&
      /nested deeper than 32/.test(error.message),
  );
  assert.equal(
    deepest(decode(fromHex(N2), PAYLOAD_DEPTH - 1), PAYLOAD_DEPTH),
    33,
  );
  assert.equal(N2.length, N1.length + 2, "N2 is not N1 one level deeper");
});

test("every example equals the committed vector of the generator, and every vector has its example", () => {
  const found = examples();
  const files = readdirSync(join(root, VECTORS)).filter((f) =>
    /^[MN]\d+\.hex$/.test(f),
  );
  assert.deepEqual(
    files.map((f) => f.replace(/\.hex$/, "")).sort(),
    Object.keys(found).sort(),
  );
  for (const [name, hex] of Object.entries(found)) {
    const vector = read(join(VECTORS, `${name}.hex`)).replace(/\s+/g, "");
    assert.equal(hex, vector, `${name} differs from ${VECTORS}/${name}.hex`);
  }
});

test("no core file calls the nesting limit or the clock's carry pending the owner", () => {
  for (const path of [
    "crates/ncdb-core/src/cbor.rs",
    "crates/ncdb-core/src/clock.rs",
    "crates/ncdb-core/tests/depth.rs",
    "crates/ncdb-core/tests/clock.rs",
  ]) {
    assert.doesNotMatch(
      read(path).replace(/\s*\/\/[!/]?\s*/g, " "),
      /pending the owner/,
      `${path} still calls a rule pending the owner`,
    );
  }
});

// NOC-40: RFC 0002 Amendment 5 records the payload key that RFC 0005's
// decisions D10 and D13 B decided: every sealed change carries its writer's
// model.id and model version in a "may ignore" key of the payload, and a
// member parks a change of another model, whole. The test checks that the
// amendment is announced and marked where it changes the RFC, that the step
// table keeps its steps and names the new reason, and that the byte-exact
// examples decode under RFC 0002's profile and equal the committed vectors
// that docs/rfc/vectors/0002/generate.py writes with Python's cbor2,
// independently of this reader and of the Rust core.
import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { decode, encode, fromHex, toHex } from "./lib/cbor-profile.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0002-change-and-log-format.md";
const VECTORS = "docs/rfc/vectors/0002";
const AMENDMENT = "amendment-5-the-writers-model-in-the-payload";
const read = (path) => readFileSync(join(root, path), "utf8");

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

/** The text of the level-2 section whose anchor is `anchor`, without the heading. */
const section = (markdown, anchor) =>
  markdown
    .split(/^(?=## )/m)
    .find((part) => slug(part.split("\n")[0].replace(/^## /, "")) === anchor)
    ?.split("\n")
    .slice(1)
    .join("\n");

/** The payload key of the amendment, as the payload tree of "A change at a glance" lists it. */
const modelKeyLine = () => {
  const glance = section(read(RFC), "a-change-at-a-glance") ?? "";
  const payload = glance.split(/^payload \(/m)[1]?.split(/\n\n/)[0] ?? "";
  return payload.split("\n").find((line) => /model/i.test(line));
};

test("RFC 0002 announces Amendment 5 in a dated changelog line at the top", () => {
  const top = read(RFC).split(/^## /m)[0];
  assert.match(
    top,
    new RegExp(
      `^- \\*\\*\\d{4}-\\d{2}-\\d{2}\\*\\* · NOC-40 · .*\\(#${AMENDMENT}\\)`,
      "m",
    ),
    "no dated changelog line for NOC-40 linking to Amendment 5",
  );
  const status = top.split("**Status.**")[1] ?? "";
  assert.match(
    status,
    new RegExp(`\\(#${AMENDMENT}\\)`),
    "the status omits Amendment 5",
  );
});

test("the amendment names where RFC 0005 uses the key, by D10 and D13", () => {
  const body = section(read(RFC), AMENDMENT);
  assert.ok(body, `no section #${AMENDMENT}`);
  assert.match(
    body,
    /\(0005-schema-and-model-lock\.md#decisions-for-the-owner\)/,
  );
  assert.match(body, /\bD10\b/);
  assert.match(body, /\bD13\b/);
});

test("the payload tree lists the model key in the may-ignore range, marked", () => {
  const line = modelKeyLine();
  assert.ok(line, "the payload tree lists no model key");
  const key = Number(line.match(/^\S+\s+(\d+) /)?.[1]);
  assert.ok(key >= 64, `the model key ${key} is not "may ignore" (64 and up)`);
  assert.match(line, /Amendment 5/, "the model key is not marked");
});

test("the step table keeps its steps, and step 6 parks a change of another model", () => {
  const amendment1 =
    section(read(RFC), "amendment-1-atomic-unit-and-rejected-changes") ?? "";
  const rows = amendment1
    .split("\n")
    .filter((line) => /^\| \d+[a-z]?\. /.test(line))
    .map((line) =>
      line
        .slice(1, -1)
        .split(" | ")
        .map((cell) => cell.trim()),
    );
  assert.deepEqual(
    rows.map(([step]) => step.split(".")[0]),
    ["1", "2", "3", "4", "5", "5a", "6", "7"],
  );
  const [, fails, state] = rows.find(([step]) => step.startsWith("6."));
  assert.match(fails, /model\.id/, "step 6 does not compare the model id");
  assert.match(
    state,
    /Parked: .*another model/,
    "step 6 names no reason for another model",
  );
  assert.match(state, /Amendment 5/, "step 6's change is not marked");
});

test("versioning and the relay's view say where the model travels", () => {
  const rfc = read(RFC);
  const versioning = section(rfc, "the-canonical-encoding") ?? "";
  assert.match(
    versioning,
    /^- \*\*A change of another model\*\*.*\*\(Amendment 5\.\)\*$/m,
    "Versioning has no marked rule for a change of another model",
  );
  const relay = section(rfc, "what-the-relay-can-see-and-what-it-cannot") ?? "";
  const model = relay
    .split("\n")
    .find((line) => line.startsWith("- **The model**"));
  assert.match(
    model ?? "",
    /inside the (sealed )?payload.*\*\(Amendment 5\.\)\*$/,
  );
});

/** The examples of Amendment 5, by name: { M1: "a2…", … }. */
const examples = () => {
  const body = section(read(RFC), AMENDMENT) ?? "";
  const part = (body.split(/^### Byte-exact examples$/m)[1] ?? "").split(
    /^### /m,
  )[0];
  const found = {};
  for (const example of part.split(/^\*\*(?=M\d+\.)/m).slice(1)) {
    const name = example.match(/^(M\d+)\./)[1];
    const hex = [...example.matchAll(/```text\n((?:[0-9a-f]+\n)+)```/g)];
    assert.equal(hex.length, 1, `${name} has ${hex.length} hex blocks, not 1`);
    found[name] = hex[0][1].replace(/\s+/g, "");
  }
  return found;
};

const entries = (map) => new Map(map.map.map(([k, v]) => [Number(k), v]));

test("the amendment has the examples M1 and M2, canonical under the profile", () => {
  const found = examples();
  assert.deepEqual(Object.keys(found), ["M1", "M2"]);
  const m1 = decode(fromHex(found.M1), MODEL_DEPTH);
  assert.equal(toHex(encode(m1)), found.M1);
  assert.deepEqual(
    [...entries(m1).keys()],
    [1, 2],
    "M1 is not {1: model id, 2: model version}",
  );
  assert.ok(entries(m1).get(1) instanceof Uint8Array);
  assert.equal(entries(m1).get(1).length, 16, "the model id is not 16 bytes");
  assert.equal(
    typeof entries(m1).get(2),
    "bigint",
    "the model version is not an integer",
  );
  const m2 = decode(fromHex(found.M2), PAYLOAD_DEPTH);
  assert.equal(toHex(encode(m2)), found.M2);
  assert.deepEqual([...entries(m2).keys()], [1, 2, 3, 4, 64]);
  assert.equal(
    toHex(encode(entries(m2).get(64))),
    found.M1,
    "M2's key 64 is not M1",
  );
});

test("every example equals the committed vector of the generator", () => {
  const rfc = examples();
  const files = existsSync(join(root, VECTORS))
    ? readdirSync(join(root, VECTORS)).filter((file) => file.endsWith(".hex"))
    : [];
  assert.deepEqual(
    files.map((file) => file.replace(/\.hex$/, "")).sort(),
    ["M1", "M2"],
    `${VECTORS} holds no vector for some example, or one the RFC lacks`,
  );
  for (const [name, hex] of Object.entries(rfc)) {
    const vector = read(join(VECTORS, `${name}.hex`)).replace(/\s+/g, "");
    assert.equal(hex, vector, `${name} differs from ${VECTORS}/${name}.hex`);
  }
});

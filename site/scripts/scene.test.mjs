// The landing page's drawing (scene.mjs): the four states say what the spec's
// table "The scenario picture" says. Needs no build.
import assert from "node:assert/strict";
import { test } from "node:test";
import { STEPS, scene } from "../src/components/scene.mjs";

const count = (s, needle) => s.split(needle).length - 1;

test("four steps with the spec's titles", () => {
  assert.deepEqual(
    STEPS.map((s) => s.title),
    [
      "Everyone holds a full copy",
      "A change travels as ciphertext",
      "The train enters a tunnel",
      "Out of the tunnel, every copy matches",
    ],
  );
});
test("state 0: three equal copies, the agent's reads circle, nothing travels to the cloud", () => {
  const s = scene(0, "a");
  assert.equal(count(s, ">open<"), 9);
  assert.equal(count(s, "<animateMotion"), 1);
  assert.equal(count(s, 'class="sc-lens"'), 0);
});
test("state 1: the lens is on the cloud and the change reaches both others", () => {
  const s = scene(1, "a");
  assert.equal(count(s, 'class="sc-lens"'), 1);
  assert.equal(count(s, "sc-row-origin"), 1);
  assert.equal(count(s, "sc-row-got"), 2);
});
test("state 2: the train is offline and its change is stored locally", () => {
  const s = scene(2, "a");
  assert.equal(count(s, "sc-dot-off"), 1);
  assert.equal(count(s, "sc-link-cut"), 1);
  assert.ok(s.includes("stored locally"));
});
test("state 3: all three copies show the same three tasks", () => {
  const s = scene(3, "a");
  for (const status of [">in progress<", ">done<", ">assigned to agent<"])
    assert.equal(count(s, status), 3, status);
  assert.equal(count(s, "sc-dot-off"), 0);
});
test("the drawing never says relay", () => {
  for (const i of [0, 1, 2, 3]) assert.ok(!/relay/i.test(scene(i, "a")));
});
test("ids carry the prefix, so two drawings can share a page", () => {
  const ids = (s) => [...s.matchAll(/\bid="([^"]+)"/g)].map((m) => m[1]);
  for (const i of [0, 1, 2, 3]) {
    assert.ok(ids(scene(i, "hero")).every((id) => id.startsWith("hero")));
    assert.equal(
      ids(scene(i, "a")).filter((id) => ids(scene(i, "b")).includes(id)).length,
      0,
    );
  }
});

// The phone drawing: the same four states in a tall layout whose labels stay readable.
for (const step of [0, 1, 2, 3]) {
  test(`state ${step}: the tall drawing shows what the wide one shows`, () => {
    const wide = scene(step, "a");
    const tall = scene(step, "a", "tall");
    assert.match(wide, /<svg class="sc sc--wide" viewBox="0 0 800 600"/);
    assert.match(tall, /<svg class="sc sc--tall" viewBox="0 0 400 704"/);
    for (const needle of [
      ">open<",
      ">in progress<",
      ">done<",
      ">assigned to agent<",
      "stored locally",
      "sc-row-origin",
      "sc-row-got",
      "sc-dot-on",
      "sc-dot-off",
      "sc-link-cut",
      'class="sc-lens"',
      "<animateMotion",
      ">Cloud server<",
      ">On the train<",
      ">Home office<",
      ">Agent<",
    ])
      assert.equal(count(tall, needle), count(wide, needle), needle);
    assert.ok(!/relay/i.test(tall));
  });
}
test("a wide and a tall drawing with one prefix share no id", () => {
  const ids = (s) => [...s.matchAll(/\bid="([^"]+)"/g)].map((m) => m[1]);
  for (const i of [0, 1, 2, 3]) {
    const tall = ids(scene(i, "a", "tall"));
    assert.ok(tall.every((id) => id.startsWith("a-tall")));
    assert.equal(
      ids(scene(i, "a")).filter((id) => tall.includes(id)).length,
      0,
    );
  }
  assert.ok(ids(scene(1, "a", "tall")).length > 0, "state 1 has the lens clip");
  assert.ok(
    ids(scene(2, "a", "tall")).length > 0,
    "state 2 has the train clip",
  );
});

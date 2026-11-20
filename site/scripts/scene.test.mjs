// The landing page's drawing (scene.mjs): the four states say what the spec's
// table "The scenario picture" says. Needs no build.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { STEPS, scene } from "../src/components/scene.mjs";

const count = (s, needle) => s.split(needle).length - 1;

test("four steps with the spec's titles", () => {
  assert.deepEqual(
    STEPS.map((s) => s.title),
    [
      "Everyone has the full task list",
      "A change travels encrypted",
      "The traveller goes offline",
      "The traveller reconnects",
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

test("the tall drawing's type sizes win over the wide ones", () => {
  const css = readFileSync(
    new URL("../src/styles/scene.css", import.meta.url),
    "utf8",
  );
  // .sc--tall .sc-x has the specificity of .sc .sc-x, which comes later and would win
  assert.ok(
    !/(^|[^.\w])\.sc--tall \.sc-/m.test(css.replace(/\.sc\.sc--tall/g, "")),
  );
  for (const part of ["label", "title", "status", "cipher", "cipher-big"])
    assert.match(css, new RegExp(`\\.sc\\.sc--tall \\.sc-${part}[ ,{]`), part);
});

test("the drawings' alt texts tell the scenario in the owner's words", () => {
  assert.deepEqual(
    STEPS.map((s) => s.alt),
    [
      "The traveller on a train, a person in a home office and an agent each hold the same three tasks. A server in the cloud sits between them.",
      "The traveller claims a task. The change travels encrypted through the server to the home office and the agent. A lens on the server shows only encrypted bytes.",
      "The traveller's train is in a tunnel and offline. The change is stored locally. The home office and the agent keep exchanging changes.",
      "The traveller is back online. Changes cross in both directions, and all three copies show the same tasks.",
    ],
  );
});

// NOC-380: the scenario's drawings stand still at every width. A still drawing shows
// the picture the moving one shows 0.72 s into its loop (as the scenario held it at
// rest before), with no animation that runs on.
for (const layout of ["wide", "tall"]) {
  for (const step of [0, 1, 2, 3]) {
    test(`state ${step}: the still ${layout} drawing does not move`, () => {
      const s = scene(step, "a", layout, { still: true });
      assert.ok(!s.includes("repeatCount"), "no loop");
      assert.ok(!s.includes("<animate "), "no fading");
      // every point it draws stands where the moving drawing has it at 0.72 s
      for (const m of s.matchAll(/<animateMotion([^>]*)\/>/g)) {
        assert.match(m[1], /fill="freeze"/);
        assert.match(m[1], /keyPoints="([\d.]+);\1"/);
      }
    });
  }
}
test("a still drawing shows the points that are on their way at 0.72 s", () => {
  const points = (s) => count(s, "<animateMotion");
  // state 0: the agent's reads circle; 1: the traveller's change goes up;
  // 2: the home office's change goes up, the agent's reads circle; 3: the traveller's change goes up
  assert.deepEqual(
    [0, 1, 2, 3].map((i) => points(scene(i, "a", "wide", { still: true }))),
    [1, 1, 2, 1],
  );
  assert.deepEqual(
    [0, 1, 2, 3].map((i) => points(scene(i, "a", "tall", { still: true }))),
    [1, 1, 2, 1],
  );
});
test("a still drawing keeps the moving one's picture apart from the motion", () => {
  const strip = (s) =>
    s
      .replace(/<circle r="\d+" class="sc-pulse"[\s\S]*?<\/circle>/g, "")
      .replace(/\s+/g, " ");
  for (const layout of ["wide", "tall"])
    for (const i of [0, 1, 2, 3])
      assert.equal(
        strip(scene(i, "a", layout, { still: true })),
        strip(scene(i, "a", layout)),
      );
});

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

// The phone drawing (owner, 2026-11-20, NOC-380): a small schematic per step with no
// text at all. The three places as icons, the server as a cloud, each copy as a small
// list of three bars, and one cue for what the step does. The step's title and text
// carry the words.
const phone = (step) => scene(step, "a", "phone");
for (const step of [0, 1, 2, 3]) {
  test(`state ${step}: the phone drawing holds no text and does not move`, () => {
    const s = phone(step);
    assert.match(s, /<svg class="sc sc--phone" viewBox="0 0 400 260"/);
    assert.equal(count(s, "<text"), 0, "text");
    assert.ok(!s.includes("<animate"), "motion");
    // three copies of three bars each, and the cloud
    assert.equal(count(s, 'class="sc-card"'), 3);
    assert.equal(count(s, 'class="sc-row'), 9);
    assert.equal(count(s, 'class="sc-cloud"'), 1);
    assert.match(
      s,
      new RegExp(`aria-label="${STEPS[step].alt.replaceAll("'", "\\'")}"`),
    );
  });
}
const flows = (s) => count(s, 'class="sc-flow"');
test("phone state 0: all three hold the same copy and nothing travels", () => {
  const s = phone(0);
  assert.equal(count(s, "sc-row-origin") + count(s, "sc-row-got"), 0);
  assert.equal(count(s, "sc-dot-on"), 3);
  assert.equal(flows(s), 0);
  assert.equal(count(s, 'class="sc-lock'), 0);
});
test("phone state 1: the traveller's change goes through the locked cloud to the other two", () => {
  const s = phone(1);
  assert.equal(count(s, "sc-row-origin"), 1);
  assert.equal(count(s, "sc-row-got"), 2);
  assert.equal(
    flows(s),
    3,
    "up from the train, down to the house and the server",
  );
  assert.ok(count(s, 'class="sc-lock') > 0, "the lock on the cloud");
});
test("phone state 2: the train is in the tunnel, its link cut, its change kept locally", () => {
  const s = phone(2);
  assert.equal(count(s, "sc-dot-off"), 1);
  assert.equal(count(s, "sc-link-cut"), 1);
  assert.equal(
    count(s, "sc-row-origin"),
    2,
    "the train's and the home office's change",
  );
  assert.equal(
    count(s, "sc-row-got"),
    1,
    "the home office's change at the agent",
  );
  assert.equal(
    flows(s),
    2,
    "the home office's change through the cloud to the agent",
  );
});
test("phone state 3: the link is back and all three copies show the same marks", () => {
  const s = phone(3);
  assert.equal(count(s, "sc-dot-off") + count(s, "sc-link-cut"), 0);
  const cards = s
    .split('class="sc-card"')
    .slice(1)
    .map((c) =>
      [...c.matchAll(/class="sc-row( sc-row-\w+)?"/g)]
        .slice(0, 3)
        .map((m) => m[1] ?? ""),
    );
  assert.equal(cards.length, 3);
  assert.deepEqual(cards[0], cards[1]);
  assert.deepEqual(cards[1], cards[2]);
  assert.ok(
    cards[0].some((mark) => mark),
    "the changes are marked",
  );
  assert.equal(flows(s), 1, "the train's link carries changes both ways");
});
test("a wide and a phone drawing with one prefix share no id", () => {
  const ids = (s) => [...s.matchAll(/\bid="([^"]+)"/g)].map((m) => m[1]);
  for (const i of [0, 1, 2, 3]) {
    const small = ids(scene(i, "a", "phone"));
    assert.ok(small.every((id) => id.startsWith("a-phone")));
    assert.equal(
      ids(scene(i, "a")).filter((id) => small.includes(id)).length,
      0,
    );
  }
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

// NOC-380: the scenario's drawings stand still at every width (the phone's has no
// motion at all, above). A still drawing shows
// the picture the moving one shows 0.72 s into its loop (as the scenario held it at
// rest before), with no animation that runs on.
for (const layout of ["wide"]) {
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
});
test("a still drawing keeps the moving one's picture apart from the motion", () => {
  const strip = (s) =>
    s
      .replace(/<circle r="\d+" class="sc-pulse"[\s\S]*?<\/circle>/g, "")
      .replace(/\s+/g, " ");
  for (const layout of ["wide"])
    for (const i of [0, 1, 2, 3])
      assert.equal(
        strip(scene(i, "a", layout, { still: true })),
        strip(scene(i, "a", layout)),
      );
});

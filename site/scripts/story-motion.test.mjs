// NOC-384 (owner, 2026-11-20): each of the scenario's four drawings animates on its
// own again, in place: it starts when it comes into view and loops by itself;
// scrolling only decides whether it is visible, never how far it has played. Under
// reduced motion every drawing holds its still picture (0.72 s into its loop), and
// the scenario's pause control stops all of its motion. Drives
// site/src/components/story-motion.mjs with stand-ins for the drawings, the buttons
// and the IntersectionObserver; scripts/site-demo/scenario-motion.e2e.test.mjs runs
// the same in Chromium.
import assert from "node:assert/strict";
import { test } from "node:test";
import { wire } from "../src/components/story-motion.mjs";

const drawing = () => ({
  time: 0,
  paused: false,
  setCurrentTime(t) {
    this.time = t;
  },
  pauseAnimations() {
    this.paused = true;
  },
  unpauseAnimations() {
    this.paused = false;
  },
});
const button = () => {
  const listeners = {};
  return {
    hidden: true,
    attrs: {},
    setAttribute(name, value) {
      this.attrs[name] = value;
    },
    addEventListener(type, fn) {
      listeners[type] = fn;
    },
    click() {
      listeners.click();
    },
  };
};
// A page of four figures, each with its drawing and its pause button.
const page = ({ reduced = false } = {}) => {
  const figures = [0, 1, 2, 3].map(() => ({
    svg: drawing(),
    button: button(),
  }));
  for (const f of figures) {
    f.querySelector = (s) => (s === "svg.sc--wide" ? f.svg : null);
  }
  const root = {
    querySelectorAll: (s) => {
      if (s === "figure") return figures;
      if (s === "[data-story-pause]") return figures.map((f) => f.button);
      return [];
    },
  };
  let observer;
  class Observer {
    constructor(callback) {
      this.callback = callback;
      this.watched = [];
      observer = this;
    }
    observe(el) {
      this.watched.push(el);
    }
  }
  wire(root, { reduced, Observer });
  const seen = (i, isIntersecting) =>
    observer.callback([{ target: figures[i], isIntersecting }]);
  return { figures, observer: () => observer, seen };
};

test("before it comes into view, a drawing stands at the start of its loop", () => {
  const { figures, observer } = page();
  for (const f of figures) {
    assert.equal(f.svg.paused, true);
    assert.equal(f.svg.time, 0);
  }
  assert.deepEqual(
    observer().watched,
    figures,
    "every figure is watched for coming into view",
  );
});

test("a drawing that comes into view plays from the start, and only it", () => {
  const { figures, seen } = page();
  seen(2, true);
  assert.equal(figures[2].svg.paused, false);
  assert.equal(figures[2].svg.time, 0);
  for (const i of [0, 1, 3]) assert.equal(figures[i].svg.paused, true, `${i}`);
});

test("out of view it holds where it is, and plays on when it comes back", () => {
  const { figures, seen } = page();
  seen(1, true);
  figures[1].svg.time = 2.5; // it has played a while
  seen(1, false);
  assert.equal(figures[1].svg.paused, true);
  seen(1, true);
  assert.equal(figures[1].svg.paused, false);
  assert.equal(figures[1].svg.time, 2.5, "not set back by the scroll");
});

test("the pause control holds every drawing, and pressed again lets those in view play", () => {
  const { figures, seen } = page();
  for (const f of figures) assert.equal(f.button.hidden, false, "shown");
  seen(0, true);
  seen(3, true);
  figures[1].button.click();
  for (const f of figures) {
    assert.equal(f.svg.paused, true);
    assert.equal(f.button.attrs["aria-pressed"], "true");
  }
  seen(2, true);
  assert.equal(figures[2].svg.paused, true, "a paused page stays paused");
  figures[3].button.click();
  for (const f of figures)
    assert.equal(f.button.attrs["aria-pressed"], "false");
  assert.equal(figures[0].svg.paused, false);
  assert.equal(figures[3].svg.paused, false);
  assert.equal(figures[2].svg.paused, false);
  assert.equal(figures[1].svg.paused, true, "out of view");
});

test("under reduced motion every drawing holds its still picture and no control shows", () => {
  const { figures, seen } = page({ reduced: true });
  for (const f of figures) {
    assert.equal(f.svg.paused, true);
    assert.equal(f.svg.time, 0.72);
    assert.equal(f.button.hidden, true);
  }
  seen(0, true);
  assert.equal(
    figures[0].svg.paused,
    true,
    "coming into view does not move it",
  );
  assert.equal(figures[0].svg.time, 0.72);
});

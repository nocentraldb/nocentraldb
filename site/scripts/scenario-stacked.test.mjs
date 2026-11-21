// NOC-380 (owner, 2026-11-20): the scenario tells its story in steps stacked one
// under the other at every width, with no scroll behaviour: nothing pins, snaps,
// takes over the wheel or moves with the scroll. On a phone each step's drawing is
// a small schematic without any text. Reads the built page in site/dist and the
// stylesheets; scripts/site-demo/scenario-stacked.e2e.test.mjs measures the same
// in Chromium.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const site = join(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(site, "dist");
const html = readFileSync(join(dist, "index.html"), "utf8");
const story = html.slice(
  html.indexOf('id="story"'),
  html.indexOf("</section>", html.indexOf('id="story"')),
);
const assets = readdirSync(join(dist, "_astro"));
const read = (ext) =>
  assets
    .filter((f) => f.endsWith(ext))
    .map((f) => readFileSync(join(dist, "_astro", f), "utf8"))
    .join("\n");
const css = `${html}\n${read(".css")}`;
const js = `${html}\n${read(".js")}`;

test("the scenario is a plain section: no moment, no snap points", () => {
  const open = html.match(/<section[^>]*id="story"[^>]*>/)[0];
  assert.ok(!open.includes("data-moment"), open);
  assert.ok(!story.includes("story-snap"), "snap points");
  assert.ok(!story.includes("mo-pc__"), "the moment's classes");
});
test("its four steps stand in one list, each with its title, text and drawing", () => {
  const steps = story.split(/<li[\s>]/).slice(1);
  assert.equal(steps.length, 4);
  for (const step of steps) {
    assert.match(step, /<h3>/);
    assert.match(step, /<p>/);
    assert.match(step, /<figure[^>]*>\s*<svg class="sc sc--wide"/);
    assert.match(step, /<svg class="sc sc--phone"/);
  }
});
// NOC-384 (owner, 2026-11-20): each step's wide drawing loops on its own again, as it
// did before NOC-380; site/scripts/story-motion.test.mjs pins when it plays.
test("each step's wide drawing loops on its own, with its pause control", () => {
  const figures = [...story.matchAll(/<figure[\s\S]*?<\/figure>/g)];
  assert.equal(figures.length, 4);
  for (const [figure] of figures) {
    const wide = figure.match(/<svg class="sc sc--wide"[\s\S]*?<\/svg>/)[0];
    assert.match(wide, /<animateMotion[^>]*repeatCount="indefinite"/);
    assert.match(
      figure,
      /<button[^>]*class="scene-pause"[^>]*aria-pressed="false"[^>]*data-story-pause[^>]*hidden/,
    );
  }
  // the hero's control (scene-pause.mjs) does not reach into the scenario
  assert.ok(!story.includes("data-scene-pause"), "the hero's pause control");
  assert.ok(!story.includes("data-scene-scope"), "the hero's scope");
});
test("the scenario's drawings start when they come into view, not by the scroll", () => {
  // the script that wires the scenario's pause control (story-motion.mjs)
  const own = assets
    .filter((f) => f.endsWith(".js"))
    .map((f) => readFileSync(join(dist, "_astro", f), "utf8"))
    .concat(
      [...html.matchAll(/<script[^>]*>([\s\S]*?)<\/script>/g)].map((m) => m[1]),
    )
    .filter((src) => src.includes("data-story-pause"));
  assert.equal(own.length, 1, "the scenario's script");
  assert.ok(own[0].includes("IntersectionObserver"), "an IntersectionObserver");
  for (const needle of [
    'addEventListener("scroll"',
    "onscroll",
    "scrollTimeline",
    "animation-timeline",
  ])
    assert.ok(!js.includes(needle), needle);
  assert.ok(!css.includes("animation-timeline"), "a scroll-driven animation");
});
test("the page loads no moment and no wheel or snap handling", () => {
  assert.ok(!html.includes("/assets/moments/"), "a moment script");
  for (const needle of [
    "scrollSnapType",
    '"wheel"',
    'addEventListener("wheel"',
  ])
    assert.ok(!js.includes(needle), needle);
  for (const needle of [
    "scroll-snap",
    ".is-live",
    "pinned-chapters",
    "story-snap",
  ])
    assert.ok(!css.includes(needle), needle);
});

// The phone rule (owner, 2026-11-20): below 640px each step shows its phone drawing,
// a schematic that holds no text; the words are the step's own.
test("each step's phone drawing holds no text", () => {
  const drawings = [
    ...story.matchAll(/<svg class="sc sc--phone"[\s\S]*?<\/svg>/g),
  ];
  assert.equal(drawings.length, 4);
  for (const [svg] of drawings)
    assert.ok(!svg.includes("<text"), svg.slice(0, 80));
});
test("below 640px a step shows its phone drawing, from 640px its wide one", () => {
  assert.match(
    css,
    /@media\s*\((?:max-width:\s*|width\s*<=\s*)639px\)\s*\{[^}]*#story figure \.sc--wide\{display:none\}/,
  );
  assert.match(css, /\.sc--phone\{display:none\}/);
});

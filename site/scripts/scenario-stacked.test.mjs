// NOC-380 (owner, 2026-11-20): the scenario tells its story in steps stacked one
// under the other at every width, with no scroll behaviour: nothing pins, snaps,
// takes over the wheel or moves with the scroll. On a phone no text in a drawing
// is larger than the step's text. Reads the built page in site/dist and the
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

test("the scenario is a plain section: no moment, no snap points, no pause control", () => {
  const open = html.match(/<section[^>]*id="story"[^>]*>/)[0];
  assert.ok(!open.includes("data-moment"), open);
  assert.ok(!story.includes("story-snap"), "snap points");
  assert.ok(!story.includes("data-scene-pause"), "pause control");
  assert.ok(!story.includes("mo-pc__"), "the moment's classes");
});
test("its four steps stand in one list, each with its title, text and drawing", () => {
  const steps = story.split(/<li[\s>]/).slice(1);
  assert.equal(steps.length, 4);
  for (const step of steps) {
    assert.match(step, /<h3>/);
    assert.match(step, /<p>/);
    assert.match(step, /<figure[^>]*>\s*<svg class="sc sc--wide"/);
    assert.match(step, /<svg class="sc sc--tall"/);
  }
});
test("its drawings stand still", () => {
  assert.ok(!story.includes("repeatCount"), "a looping animation");
  assert.ok(!story.includes("<animate "), "a fading point");
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

// The phone rule. A drawing's text is drawn at its font size times the drawing's
// width over its viewBox width. The tall drawing (viewBox 400 wide) is at most 400 px
// wide on a phone, and none of its type is set above the step's 16 px body text.
const BODY = 16;
test("on a phone no text in a drawing is set larger than the step's text", () => {
  const scene = readFileSync(join(site, "src/styles/scene.css"), "utf8");
  const sizes = [
    ...scene.matchAll(
      /\.sc\.sc--tall \.sc-[\w-]+\s*\{\s*font-size:\s*([\d.]+)px/g,
    ),
  ].map((m) => Number(m[1]));
  assert.equal(sizes.length, 5, "the tall drawing's five type sizes");
  for (const size of sizes) assert.ok(size <= BODY, `${size}px`);
});
test("on a phone a drawing is never wider than its viewBox", () => {
  assert.match(
    css,
    /@media\s*\((?:max-width:\s*|width\s*<=\s*)639px\)\s*\{[^}]*#story figure\{[^}]*max-width:400px/,
  );
});

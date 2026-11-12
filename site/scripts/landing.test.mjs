// Checks the built landing page in site/dist/index.html: the shell on the
// vendored design kit, as the landing-page tasks build it up.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const html = readFileSync("site/dist/index.html", "utf8");

test("the page declares its kind and loads the kit", () => {
  assert.match(html, /<html[^>]*data-type="software"[^>]*data-motion/);
  assert.match(html, /<meta name="site-kind" content="software"/);
  assert.match(html, /href="\/assets\/system\.css"/);
  assert.match(html, /src="\/assets\/motion\/motion\.js"/);
});
test("the header shows the wordmark and four links", () => {
  assert.match(html, /<svg[^>]*role="img"[^>]*aria-label="NoCentralDB"/);
  for (const label of ["How it works", "Code", "Docs", "GitHub"])
    assert.ok(html.includes(`>${label}<`), label);
});
test("the title is unchanged and the description is the subline", () => {
  assert.match(
    html,
    /<title>NoCentralDB: a local-first database for people and agents<\/title>/,
  );
  assert.ok(
    html.includes(
      "Each person and each agent reads and writes a full copy on their own machine.",
    ),
  );
});

import { STEPS } from "../src/components/scene.mjs";

test("the hero says what it is, in the spec's words", () => {
  assert.match(html, /<h1[^>]*>A database that reads like a local file and syncs like a shared doc<\/h1>/);
  for (const line of [
    "A local-first database for people and agents",
    "The changes sync encrypted through a server that can't read them.",
    "NoCentralDB does not exist yet.",
    "SDKs for TypeScript and Rust.",
    "See how it works",
    "Review the RFCs",
  ]) assert.ok(html.includes(line.replace("'", "&#39;")) || html.includes(line), line);
});
test("the scenario holds all four steps and five drawings in the HTML", () => {
  assert.match(html, /<section[^>]*id="story"[^>]*data-moment="pinned-chapters"/);
  for (const s of STEPS) assert.ok(html.includes(s.title), s.title);
  assert.equal(html.split('class="sc"').length - 1, 5);
  assert.equal(html.split("mo-pc__step\"").length - 1, 4);
});
test("hero and scenario never say relay, and the drawings can be paused", () => {
  const upTo = html.indexOf('id="code"');
  assert.ok(!/relay/i.test(html.slice(html.indexOf("<main"), upTo)));
  assert.ok(html.includes("data-scene-pause"));
});
test("the removed sections are gone", () => {
  for (const id of ['id="use-cases"', 'id="live"']) assert.ok(!html.includes(id), id);
});
test("the code section shows the task model in five tabs", () => {
  const code = html.slice(html.indexOf('id="code"'));
  assert.equal(code.split('role="tab"').length - 1, 5);
  for (const m of code.matchAll(/role="tab"[^>]*aria-controls="([^"]+)"/g)) assert.ok(code.includes(`id="${m[1]}"`), m[1]);
  assert.ok(code.includes("byStatusPriority"));
  assert.ok(!code.includes("nc.collection(\n    {\n      text"), "the chat model is gone");
  assert.ok(code.includes("The relay is the server from the scenario above."));
});
test("the close sits on the yellow ground and links to the docs and the use cases", () => {
  assert.match(html, /<footer[^>]*class="[^"]*closing/);
  assert.ok(html.includes("Read the design, then tell us where it breaks"));
  for (const href of ['href="/docs/"', 'href="/docs/api/"', 'href="/docs/rfcs/"', "docs/use-cases.md", "/discussions"]) assert.ok(html.includes(href), href);
  assert.ok(html.includes("Initiated by nxsflow."));
  assert.ok(!html.includes('id="limits"'), "the limits section is gone");
});
test("magenta appears once in the page and the stylesheets it loads", () => {
  // The page's HTML (with its inline <style>) and the /_astro/*.css it links;
  // the docs' own starlight.css is not loaded here and keeps its --loud.
  const linked = [...html.matchAll(/<link[^>]*href="\/_astro\/([^"]+\.css)"/g)].map((m) => readFileSync(`site/dist/_astro/${m[1]}`, "utf8"));
  const all = [html, ...linked].join("");
  assert.equal(all.split(/ff3d8b/i).length - 1, 1);
});

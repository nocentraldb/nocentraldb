// Checks the built landing page's live demo (NOC-319) without a browser:
// the section is in the page with its waiting text, and nothing of the
// demo is in the first render: no script, stylesheet or preload names a
// file under /demo/, so the browser fetches none of it before the inline
// loader asks for it after the page's load event.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const dist = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");
const html = readFileSync(join(dist, "index.html"), "utf8");

test("the landing page holds the demo section and its waiting state, and no more", () => {
  assert.match(html, /<section[^>]*id="demo"/);
  assert.match(html, /<[^>]*data-demo-status[^>]*data-state="waiting"/);
  // The task list comes with the demo, after the page has rendered.
  assert.doesNotMatch(html, /id="demo-add"/);
  assert.doesNotMatch(html, /id="demo-open"/);
});

test("nothing under /demo/ is part of the first render", () => {
  assert.doesNotMatch(html, /<script[^>]*\ssrc="\/demo\//);
  assert.doesNotMatch(html, /<link[^>]*href="\/demo\//);
  assert.doesNotMatch(html, /modulepreload[^>]*\/demo\//);
});

test("the loader starts after the load event, near the viewport and when the browser is idle", () => {
  assert.match(html, /<script type="module" data-demo-loader>/, "a module script, which never blocks parsing");
  const loader = html.slice(html.indexOf("data-demo-loader"));
  assert.match(loader, /addEventListener\(\s*"load"/);
  assert.match(loader, /IntersectionObserver/);
  assert.match(loader, /requestIdleCallback/);
  assert.match(loader, /import\(/);
});

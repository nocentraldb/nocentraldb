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
  assert.match(
    html,
    /<script type="module" data-demo-loader>/,
    "a module script, which never blocks parsing",
  );
  const loader = html.slice(html.indexOf("data-demo-loader"));
  assert.match(loader, /addEventListener\(\s*"load"/);
  assert.match(loader, /IntersectionObserver/);
  assert.match(loader, /requestIdleCallback/);
  assert.match(loader, /import\(/);
});

test("a page loaded in the background starts the demo when idle, without waiting to be in view", () => {
  // A hidden page gets no IntersectionObserver callback until it is shown,
  // so a tab opened in the background would start the demo only then
  // (the owner's report on #333). It has no first paint to protect.
  const loader = html.slice(html.indexOf("data-demo-loader"));
  assert.match(loader, /document\.visibilityState\s*===\s*"hidden"/);
});

test("the hint says that on a phone the other tab catches up when shown", () => {
  // Phones keep one tab on screen and freeze the others (the owner's
  // report on #333, Arc on iOS), so the hint promises no live update there.
  const section = html.slice(html.indexOf('id="demo"'));
  const hint = section.slice(
    section.indexOf("demo__hint"),
    section.indexOf("</p>", section.indexOf("demo__hint")),
  );
  assert.match(hint, /phone/);
  assert.match(hint, /catches up/);
});

test("in a private window the demo says why it does not start, and does not suggest a reload", () => {
  // nc.open refuses a browser without OPFS with storageUnavailable: a
  // private window of Firefox or Safari (NOC-347). Reloading never helps there.
  const loader = html.slice(html.indexOf("data-demo-loader"));
  assert.match(loader, /"storageUnavailable"/);
  assert.match(loader, /private window/);
  assert.match(loader, /normal window/);
});

test("the hint names Safari's limit with a frozen tab", () => {
  const section = html.slice(html.indexOf('id="demo"'));
  const hint = section.slice(
    section.indexOf("demo__hint"),
    section.indexOf("</p>", section.indexOf("demo__hint")),
  );
  assert.match(hint, /Safari/);
  assert.match(hint, /wait/);
});

test("the hint invites the visitor to watch the Network tab, and says why a second window follows", () => {
  const section = html.slice(html.indexOf('id="demo"'));
  const hint = section
    .slice(
      section.indexOf("demo__hint"),
      section.indexOf("</p>", section.indexOf("demo__hint")),
    )
    .replace(/<[^>]*>/g, "")
    .replace(/\s+/g, " ");
  assert.match(hint, /developer tools/);
  assert.match(hint, /Network tab/);
  assert.match(hint, /sends no request/);
  // it is the one database of this browser, not a sync, that the windows share
  assert.match(hint, /share one database in this browser/);
  assert.match(hint, /another browser/);
});

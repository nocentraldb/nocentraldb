// Checks the built landing page in site/dist/index.html: the shell on the
// vendored design kit, as the landing-page tasks build it up.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const dist = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");
const html = readFileSync(join(dist, "index.html"), "utf8");

test("the page declares its kind and loads the kit", () => {
  assert.match(html, /<html[^>]*data-type="software"[^>]*data-motion/);
  assert.match(html, /<meta name="site-kind" content="software"/);
  assert.match(html, /href="\/assets\/system\.css"/);
  assert.match(html, /src="\/assets\/motion\/motion\.js"/);
});
test("the header shows the mark, the wordmark and four links", () => {
  // the yellow mark beside the wordmark (owner, 2026-11-19); the link carries the name
  assert.match(
    html,
    /<a class="nav__logo" href="#top" aria-label="NoCentralDB">/,
  );
  assert.match(
    html,
    /<img class="nav__mark" src="\/brand\/ncdb-mark-accent-512\.png" alt="" width="32" height="32"/,
  );
  assert.match(html, /<span class="nav__wordmark"><svg[^>]*aria-hidden="true"/);
  for (const label of ["How it works", "Code", "Docs", "GitHub"])
    assert.ok(html.includes(`>${label}<`), label);
});
test("the title is unchanged and the description is the subline", () => {
  assert.match(
    html,
    /<meta name="description" content="The database puts all the data right next to the people and agents who use it\. A server keeps everyone in sync, yet it only ever sees encrypted data\."/,
  );
  assert.match(
    html,
    /<title>NoCentralDB: a local-first database for people and agents<\/title>/,
  );
  assert.ok(
    html.includes(
      "The database puts all the data right next to the people and agents who use it. A server keeps everyone in sync, yet it only ever sees encrypted data.",
    ),
  );
});

import { STEPS } from "../src/components/scene.mjs";
import { currentInstallLines } from "../src/install.mjs";

test("the hero says what it is, in the spec's words", () => {
  assert.match(
    html,
    /<h1[^>]*>Reads like local files, syncs like shared docs<\/h1>/,
  );
  for (const line of [
    "A local-first database for people and agents",
    "A server keeps everyone in sync, yet it only ever sees encrypted data.",
    "NoCentralDB does not exist yet.",
    "A Rust SDK, and a TypeScript SDK for the browser: Chromium, Firefox and Safari. One Rust core, natively and as WebAssembly. Open source under MIT or Apache-2.0.",
    "See how it works",
    "Get started",
  ])
    assert.ok(
      html.includes(line.replace("'", "&#39;")) || html.includes(line),
      line,
    );
});
test("the scenario's title is the owner's", () => {
  assert.match(
    html,
    /<h2 id="story-h">Two people, one of them travelling, and an agent work on the same task list<\/h2>/,
  );
});
test("the scenario holds all four steps and their drawings in the HTML", () => {
  assert.match(html, /<section[^>]*id="story"/);
  for (const s of STEPS) assert.ok(html.includes(s.title), s.title);
  assert.equal(html.split('class="sc sc--wide"').length - 1, 5);
  // each of the scenario's four steps also carries the schematic a phone shows
  assert.equal(html.split('class="sc sc--phone"').length - 1, 4);
  assert.equal(html.split('story__step"').length - 1, 4);
});
test("hero and scenario never say relay, and the hero's drawing can be paused", () => {
  const upTo = html.indexOf('id="code"');
  assert.ok(!/relay/i.test(html.slice(html.indexOf("<main"), upTo)));
  assert.ok(html.includes("data-scene-pause"));
});
test("the removed sections are gone", () => {
  for (const id of ['id="use-cases"', 'id="live"'])
    assert.ok(!html.includes(id), id);
});
test("the code section shows installing and the task model in seven tabs", () => {
  const code = html.slice(html.indexOf('id="code"'));
  assert.equal(code.split('role="tab"').length - 1, 7);
  for (const m of code.matchAll(/role="tab"[^>]*aria-controls="([^"]+)"/g))
    assert.ok(code.includes(`id="${m[1]}"`), m[1]);
  assert.ok(code.includes("byStatusPriority"));
  // the section's text without its tags: the highlighter splits the code into spans
  const text = code
    .slice(0, code.indexOf("</section>"))
    .replace(/<[^>]*>/g, "");
  for (const word of ["sentAt", "messages"])
    assert.ok(!text.includes(word), `the chat model is gone: ${word}`);
  assert.ok(code.includes("The relay is the server from the scenario above."));
});
test("the close sits on the yellow ground and links to the docs and the use cases", () => {
  assert.match(html, /<footer[^>]*class="[^"]*closing/);
  assert.ok(html.includes("Build with 0.1, then tell us where it breaks"));
  for (const href of [
    'href="/docs/"',
    'href="/docs/api/"',
    'href="/docs/rfcs/"',
    "docs/use-cases.md",
    "/discussions",
  ])
    assert.ok(html.includes(href), href);
  assert.ok(html.includes("Initiated by nxsflow."));
  assert.ok(!html.includes('id="limits"'), "the limits section is gone");
});
test("magenta appears once in the page and the stylesheets it loads", () => {
  // The page's HTML (with its inline <style>) and the /_astro/*.css it links;
  // the docs' own starlight.css is not loaded here and keeps its --loud.
  const linked = [
    ...html.matchAll(/<link[^>]*href="\/_astro\/([^"]+\.css)"/g),
  ].map((m) => readFileSync(join(dist, "_astro", m[1]), "utf8"));
  const all = [html, ...linked].join("");
  assert.equal(all.split(/ff3d8b/i).length - 1, 1);
});
test("the close's yellow is the footer's own ground, edge to edge", () => {
  assert.match(html, /<footer[^>]*class="[^"]*closing[^"]*ground-brand/);
  assert.ok(!html.includes("100vmax"), "no spread-shadow band");
});
test("the hero's pause control keeps one name and says its state with aria-pressed", () => {
  // the scenario's drawings stand still and need none (NOC-380)
  const buttons = [...html.matchAll(/<button[^>]*data-scene-pause[^>]*>/g)].map(
    (m) => m[0],
  );
  assert.equal(buttons.length, 1);
  for (const b of buttons) {
    assert.match(b, /aria-label="Pause the animation"/);
    assert.match(b, /aria-pressed="false"/);
  }
});
test("below a desktop the header opens its links from a menu button", () => {
  assert.match(
    html,
    /<button class="nav__toggle"[^>]*aria-expanded="false"[^>]*aria-label="Menu"/,
  );
});
test("the code tabs are wired by the kit alone, without a dead data-tabs hook", () => {
  assert.ok(!html.includes("data-tabs"));
});

test("scenario step 1 is the owner's text", () => {
  const step = html.split('story__step"')[1].replaceAll("&#39;", "'");
  assert.ok(
    step.includes("<h3>Everyone has the full task list</h3>"),
    "heading",
  );
  assert.ok(
    step.includes(
      "The traveller, the person in the home office and the agent in the cloud each keep the whole task list on their own disk. Everyone reads locally; reading needs no network connection.",
    ),
    "body",
  );
});

test("scenario step 2 is the owner's text", () => {
  const step = html.split('story__step"')[2].replaceAll("&#39;", "'");
  assert.ok(step.includes("<h3>A change travels encrypted</h3>"), "heading");
  assert.ok(
    step.includes(
      "The traveller claims a task. The change goes through the server to the other two. The server sees only encrypted bytes and has no key to read them.",
    ),
    "body",
  );
});

test("scenario step 3 is the owner's text", () => {
  const step = html.split('story__step"')[3].replaceAll("&#39;", "'");
  assert.ok(step.includes("<h3>The traveller goes offline</h3>"), "heading");
  assert.ok(
    step.includes(
      "The train enters a tunnel, but the traveller keeps working; the changes are stored locally. The other two carry on from the last state they know and don't see the traveller's changes yet.",
    ),
    "body",
  );
});

test("scenario step 4 is the owner's text", () => {
  const step = html.split('story__step"')[4].replaceAll("&#39;", "'");
  assert.ok(step.includes("<h3>The traveller reconnects</h3>"), "heading");
  assert.ok(
    step.includes(
      "The traveller's changes go out and the others' changes come in. All three copies end up in the same state.",
    ),
    "body",
  );
});

test("on a phone the header shows only the mark", () => {
  assert.match(
    html,
    /@media\s*\((?:max-width:\s*|width\s*<=\s*)639px\)\s*\{\s*\.nav__wordmark\s*\{\s*display:\s*none/,
  );
});

test("the code section's heading is the owner's \"Get started\"", () => {
  assert.match(html, /<h2 id="code-h"[^>]*>Get started<\/h2>/);
});

test("a Read step, a one-off query, comes between Write and Subscribe", () => {
  const code = html.slice(html.indexOf('id="code"'));
  const order = [...code.matchAll(/role="tab"[^>]*id="tab-([a-z]+)"/g)].map(
    (m) => m[1],
  );
  assert.deepEqual(order, [
    "install",
    "model",
    "open",
    "share",
    "write",
    "read",
    "subscribe",
  ]);
  const panel = code.slice(code.indexOf('id="panel-read"'));
  const text = panel
    .slice(0, panel.indexOf('role="tabpanel"', 10))
    .replace(/<[^>]*>/g, "");
  assert.ok(
    text.includes("await db.find(query)"),
    "the read panel queries once",
  );
});

test("Install is the first step, and no install line stands beside the tabs", () => {
  const code = html.slice(html.indexOf('id="code"'));
  assert.ok(
    !code.includes('class="install'),
    "the separate install line is gone",
  );
  const panel = code.slice(code.indexOf('id="panel-install"'));
  const text = panel
    .slice(0, panel.indexOf('role="tabpanel"', 10))
    .replace(/<[^>]*>/g, "")
    .replace(/&#x2F;|&#47;/g, "/");
  // the lines install.mjs picks for this repository; install.test.mjs covers each state
  const lines = currentInstallLines(new URL("../../", import.meta.url));
  assert.ok(text.includes(lines.npm), lines.npm);
  assert.ok(text.includes(lines.cargo), lines.cargo);
  if (lines.stage === "alpha")
    assert.ok(
      code.includes("github.com/nocentraldb/nocentraldb/releases"),
      "the releases page",
    );
});

test("a TypeScript and Rust switch is a radio group that changes every step", () => {
  const code = html.slice(
    html.indexOf('id="code"'),
    html.indexOf("</section>", html.indexOf('id="code"')),
  );
  assert.match(code, /role="radiogroup"[^>]*aria-label="Language"/);
  assert.match(
    code,
    /<input type="radio" name="code-lang" value="ts"[^>]*checked/,
  );
  assert.match(code, /<input type="radio" name="code-lang" value="rust"/);
  const panels = code.split('role="tabpanel"').slice(1);
  assert.equal(panels.length, 7);
  for (const panel of panels) {
    assert.ok(panel.includes('data-lang="ts"'), "a TypeScript variant");
    assert.ok(panel.includes('data-lang="rust"'), "a Rust variant");
  }
  const text = code.replace(/<[^>]*>/g, "").replace(/&amp;|&#x26;/g, "&");
  for (const call of [
    "Db::open(options)",
    ".members().add(",
    ".write(|tx|",
    "db.find(&query)",
    "db.subscribe(query)",
  ])
    assert.ok(text.includes(call), call);
  assert.ok(!/^# /m.test(text), "a hidden rustdoc line reached the page");
});

test("the language choice is remembered per viewer, and a blocked storage never breaks the page", () => {
  const script = html.slice(html.indexOf("data-code-lang-store"));
  assert.match(script, /localStorage/);
  assert.match(script, /try\s*\{/);
});

test("the Rust snippets are compiled by the doc tests of nocentraldb", () => {
  const lib = readFileSync(
    join(dist, "..", "..", "crates", "nocentraldb", "src", "lib.rs"),
    "utf8",
  );
  assert.ok(
    lib.includes('include_str!("../../../site/src/components/code-rust.md")'),
  );
});

test("the code section says the SDK handles every key, and names the one secret an app passes on", () => {
  const code = html.slice(html.indexOf('id="code"'));
  const head = code.slice(0, code.indexOf('class="lang-switch'));
  const text = head.replace(/<[^>]*>/g, "");
  assert.ok(
    text.includes(
      "The SDK handles all the encryption. Your code never touches a key.",
    ),
  );
  assert.match(text, /recovery code/);
  assert.ok(
    head.includes("docs/getting-started-browser.md#16-remove-a-lost-device"),
    "the guide's recovery section",
  );
  assert.ok(!text.includes("designed to"), "the old hedge is gone");
});

test("the fact line promises no mobile SDK, which 0.1 does not ship", () => {
  const hero = html.slice(
    html.indexOf('<section class="hero'),
    html.indexOf("</section>"),
  );
  assert.ok(!/mobile/i.test(hero), "the hero names mobile");
});

test("the hero's second button leads to the docs, not the RFCs", () => {
  const hero = html.slice(
    html.indexOf('<section class="hero'),
    html.indexOf("</section>"),
  );
  assert.match(
    hero,
    /<a class="btn btn--secondary" href="\/docs\/"\s*>Get started<\/a/,
  );
  assert.ok(!hero.includes("Review the RFCs"));
});

test("the close invites the visitor to contribute, beside the feedback", () => {
  const close = html.slice(html.indexOf("<footer"));
  for (const href of [
    "https://github.com/nocentraldb/nocentraldb/discussions",
    "https://github.com/nocentraldb/nocentraldb/blob/main/CONTRIBUTING.md",
    "https://github.com/nocentraldb/nocentraldb/labels/good%20first%20issue",
  ])
    assert.ok(close.includes(`href="${href}"`), href);
  assert.match(close, />Contribute</);
  assert.match(close, />Good first issues</);
});

test("the close's API card describes the calls 0.1 ships, not a design", () => {
  const close = html.slice(html.indexOf("<footer"));
  assert.ok(!close.includes("designs them"));
  assert.ok(
    close.replaceAll("&#39;", "'").includes("The SDK's main calls in 0.1."),
  );
});

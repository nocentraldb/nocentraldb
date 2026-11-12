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

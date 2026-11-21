// The docs' header shows the yellow mark beside the wordmark, and only the mark
// on a phone, as the landing page's header does (owner, 2026-11-19). The link
// keeps the name NoCentralDB.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const dist = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");
const html = readFileSync(join(dist, "docs/index.html"), "utf8");

test("the docs' site title holds the mark and the wordmark, and keeps the name", () => {
  const start = html.indexOf('class="site-title');
  assert.ok(start >= 0, "no site title");
  const title = html.slice(start, html.indexOf("</a>", start));
  assert.match(
    title,
    /<img[^>]*class="[^"]*ncdb-mark[^"]*"[^>]*src="\/brand\/ncdb-mark-accent-512\.png"/,
  );
  assert.match(title, /<img[^>]*class="[^"]*ncdb-wordmark/);
  assert.match(title, /NoCentralDB<\/span>/);
});

test("on a phone the docs' header shows only the mark", () => {
  assert.match(
    html,
    /@media\s*\((?:max-width:\s*|width\s*<=\s*)639px\)\s*\{[^}]*\.ncdb-wordmark[^{]*\{\s*display:\s*none/,
  );
});

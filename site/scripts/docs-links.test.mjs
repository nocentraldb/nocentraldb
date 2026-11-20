// The docs' links, in the built site (NOC-361): the developer docs are pages
// of the site, rendered from docs/, so a link to one stays on the site; no
// link of the site breaks, its anchor included; and a link to a file of the
// repository on GitHub names a file or folder that exists on this commit.
// The sidebar lists the two guides right after "Getting started" and
// "Overview" (the owner's direction of 2026-11-19).
import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { REPO_DOCS } from "../src/repo-docs.mjs";

const site = join(dirname(fileURLToPath(import.meta.url)), "..");
const repo = join(site, "..");
const dist = join(site, "dist");
const GIT = "https://github.com/nocentraldb/nocentraldb/";

/** Every built HTML page, relative to dist. */
const pages = (dir = dist) =>
  readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return pages(path);
    return entry.name.endsWith(".html") ? [relative(dist, path)] : [];
  });

const html = new Map(
  pages().map((p) => [p, readFileSync(join(dist, p), "utf8")]),
);

/** The hrefs of a page's content, without its head, header and sidebar. */
const hrefs = (text) =>
  [...text.matchAll(/<a\b[^>]*\bhref="([^"]*)"/g)].map((m) =>
    m[1].replaceAll("&amp;", "&"),
  );

/** The built file a site path is served from, or undefined. */
const served = (path) =>
  [path, join(path, "index.html"), `${path}.html`].find(
    (p) =>
      html.has(p) ||
      (p !== "" && !p.endsWith("/") && existsSync(join(dist, p))),
  );

const ids = (text) =>
  new Set([...text.matchAll(/\bid="([^"]*)"/g)].map((m) => m[1]));

test("every link within the site leads to a built page, and its anchor exists", () => {
  const broken = [];
  for (const [page, text] of html) {
    for (const href of hrefs(text)) {
      if (!href.startsWith("/") && !href.startsWith("#")) continue;
      const [path, anchor] = href.startsWith("#")
        ? [`/${page}`, href.slice(1)]
        : href.split("#");
      const file = served(decodeURI(path).replace(/^\//, ""));
      if (!file) {
        broken.push(`${page}: ${href}`);
        continue;
      }
      if (
        anchor &&
        html.has(file) &&
        !ids(html.get(file)).has(decodeURIComponent(anchor))
      )
        broken.push(`${page}: ${href} (no anchor)`);
    }
  }
  assert.deepEqual(broken, []);
});

test("every link to the repository on GitHub names a file or folder of this commit", () => {
  const broken = [];
  for (const [page, text] of html)
    for (const href of hrefs(text)) {
      const m = href.match(
        /^https:\/\/github\.com\/nocentraldb\/nocentraldb\/(?:blob|tree)\/main\/([^#?]*)/,
      );
      if (m && !existsSync(join(repo, decodeURI(m[1]))))
        broken.push(`${page}: ${href}`);
    }
  assert.deepEqual(broken, []);
});

test("a link to a doc the site renders stays on the site", () => {
  const out = [];
  for (const [page, text] of html)
    for (const href of hrefs(text))
      for (const doc of REPO_DOCS)
        if (href.startsWith(`${GIT}blob/main/docs/${doc.file}`))
          out.push(`${page}: ${href}`);
  assert.deepEqual(out, []);
});

test("every rendered doc is a page with its Markdown copy", () => {
  for (const doc of REPO_DOCS) {
    assert.ok(html.has(`${doc.slug}/index.html`), doc.slug);
    assert.ok(existsSync(join(dist, `${doc.slug}.md`)), `${doc.slug}.md`);
  }
});

test("the sidebar lists the two guides right after Getting started and Overview", () => {
  const text = html.get("docs/index.html");
  const nav = text.slice(text.indexOf('id="starlight__sidebar"'));
  const order = [...nav.matchAll(/<a href="(\/docs\/[^"]*)"/g)].map(
    (m) => m[1],
  );
  assert.deepEqual(order.slice(0, 4), [
    "/docs/",
    "/docs/how-it-works/",
    `/${REPO_DOCS[0].slug}/`,
    `/${REPO_DOCS[1].slug}/`,
  ]);
  for (const doc of REPO_DOCS)
    assert.ok(order.includes(`/${doc.slug}/`), doc.slug);
  assert.deepEqual(
    REPO_DOCS.slice(0, 2).map((doc) => doc.file),
    ["getting-started.md", "getting-started-browser.md"],
  );
});

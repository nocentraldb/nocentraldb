// Checks the built site in site/dist (NOC-213), as `repo:site` runs it after
// `pnpm --dir site build`: the landing page and the docs are there, the
// llms.txt files of starlight-llms-txt are served and name the site's
// domain, every docs page has its Markdown copy at its path plus `.md`, and
// the CNAME names nocentraldb.io.
import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const site = join(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(site, "dist");
const docs = join(site, "src/content/docs");
const DOMAIN = "nocentraldb.io";

/** The text of the built file `path`, which must exist and not be empty. */
const built = (path) => {
  const file = join(dist, path);
  assert.ok(existsSync(file), `site/dist/${path} is missing`);
  const text = readFileSync(file, "utf8");
  assert.ok(text.trim().length > 0, `site/dist/${path} is empty`);
  return text;
};

/** Every Markdown source under the docs collection, relative to it. */
const sources = (dir = docs) =>
  readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return sources(path);
    return /\.mdx?$/.test(entry.name) ? [relative(docs, path)] : [];
  });

/** Where the Markdown copy of the docs source `source` is served. */
const copyOf = (source) =>
  `${source.replace(/\.mdx?$/, "").replace(/(^|\/)index$/, "")}.md`;

test("the landing page and the docs are built", () => {
  assert.match(built("index.html"), /<title>NoCentralDB/);
  built("docs/index.html");
});

test("llms.txt and llms-full.txt are served and name the site's domain", () => {
  assert.ok(built("llms.txt").includes(`https://${DOMAIN}/`));
  built("llms-full.txt");
});

test("every docs page has a Markdown copy at its path plus .md", () => {
  const pages = sources();
  assert.ok(pages.length > 0, "the docs collection has no page");
  for (const page of pages) {
    const copy = built(copyOf(page));
    assert.match(copy, /^# /, `${copyOf(page)} has no title`);
  }
});

test("the CNAME names the custom domain", () => {
  assert.equal(built("CNAME").trim(), DOMAIN);
});

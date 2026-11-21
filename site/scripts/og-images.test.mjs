// Checks the Open Graph images in the built site (NOC-410): every page names
// its image with an absolute URL, its size and its alt text, and asks for a
// large card on X; the landing page has its own image and every other page
// shares the docs' image; each image is in site/dist, a 1200x630 PNG well
// under 1 MB, and was rendered from the source as it is now
// (scripts/og-images.mjs writes the hash of its HTML into the PNG).
import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { OG_IMAGES, ogUrl } from "../src/og.mjs";
import { ogHtml, pngInfo, sourceHash } from "./og-images.mjs";

const dist = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");

/** Every built HTML page, relative to dist, without the redirect pages. */
const pages = (dir = dist) =>
  readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return pages(path);
    if (!entry.name.endsWith(".html")) return [];
    const html = readFileSync(path, "utf8");
    return /http-equiv="refresh"/.test(html) ? [] : [relative(dist, path)];
  });

/** The content of the meta tag whose property or name is `key`, or undefined. */
const meta = (html, key) => {
  for (const [tag] of html.matchAll(/<meta\b[^>]*>/g)) {
    if (!new RegExp(`(property|name)="${key}"`).test(tag)) continue;
    return tag.match(/content="([^"]*)"/)?.[1];
  }
  return undefined;
};

const unescape = (text) =>
  text.replace(/&#39;/g, "'").replace(/&quot;/g, '"').replace(/&amp;/g, "&");

test("every page sets og:image, its size and alt text, and a large card", () => {
  const built = pages();
  assert.ok(built.length > 2, "site/dist holds no pages; build the site first");
  for (const page of built) {
    const html = readFileSync(join(dist, page), "utf8");
    const image = page === "index.html" ? OG_IMAGES.landing : OG_IMAGES.docs;
    assert.equal(meta(html, "og:image"), ogUrl(image), `${page}: og:image`);
    assert.equal(meta(html, "og:image:width"), "1200", `${page}: og:image:width`);
    assert.equal(meta(html, "og:image:height"), "630", `${page}: og:image:height`);
    assert.equal(unescape(meta(html, "og:image:alt") ?? ""), image.alt, `${page}: og:image:alt`);
    assert.equal(meta(html, "twitter:card"), "summary_large_image", `${page}: twitter:card`);
  }
});

test("the landing page keeps its title and description", () => {
  const html = readFileSync(join(dist, "index.html"), "utf8");
  assert.match(html, /<title>NoCentralDB: a local-first database for people and agents<\/title>/);
  assert.match(meta(html, "description") ?? "", /^The database puts all the data right next to/);
});

test("the landing page and the docs have separate images", () => {
  assert.notEqual(OG_IMAGES.landing.file, OG_IMAGES.docs.file);
  assert.match(ogUrl(OG_IMAGES.landing), /^https:\/\/nocentraldb\.io\/.+\.png$/);
});

for (const [name, image] of Object.entries(OG_IMAGES)) {
  test(`the ${name} image is a 1200x630 PNG under 1 MB, rendered from its current source`, () => {
    const file = join(dist, image.file);
    assert.ok(existsSync(file), `site/dist/${image.file} is missing`);
    const bytes = readFileSync(file);
    assert.ok(bytes.length < 1_000_000, `${image.file} is ${bytes.length} bytes`);
    const info = pngInfo(bytes);
    assert.equal(info.width, 1200);
    assert.equal(info.height, 630);
    assert.equal(
      info.text.source,
      sourceHash(ogHtml(image)),
      `${image.file} is stale: run node site/scripts/og-images.mjs`,
    );
  });
}

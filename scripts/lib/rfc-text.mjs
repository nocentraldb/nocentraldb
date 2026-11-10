// Helpers for the tests that read an RFC's Markdown: its headings, a section
// by its anchor, its relative links, and whether each link and anchor
// resolves. GitHub's heading slugs are what the anchors follow.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

export const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

export const read = (path) => readFileSync(join(root, path), "utf8");

/** GitHub's heading slug: lower case, punctuation dropped, spaces to dashes. */
export const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N} _-]/gu, "")
    .replace(/ /g, "-");

/** The headings of `markdown`, outside code blocks, without their `#`s. */
export const headings = (markdown) => {
  let fenced = false;
  const found = [];
  for (const line of markdown.split("\n")) {
    if (line.startsWith("```")) fenced = !fenced;
    else if (!fenced && /^#{1,6} /.test(line))
      found.push(line.replace(/^#+ /, ""));
  }
  return found;
};

/** The text under the heading whose anchor is `anchor`, up to the next heading of its level or higher. */
export const section = (markdown, anchor) => {
  const lines = markdown.split("\n");
  const start = lines.findIndex(
    (l) => /^#{1,6} /.test(l) && slug(l.replace(/^#+ /, "")) === anchor,
  );
  assert.ok(start >= 0, `no heading #${anchor}`);
  const level = lines[start].match(/^#+/)[0].length;
  const rest = lines.slice(start + 1);
  let fenced = false;
  const end = rest.findIndex((l) => {
    if (l.startsWith("```")) fenced = !fenced;
    const m = !fenced && l.match(/^(#+) /);
    return m && m[1].length <= level;
  });
  return (end < 0 ? rest : rest.slice(0, end)).join("\n");
};

/** Every relative link target in `markdown`, such as `0003-keys.md#the-keys` or `#history`. */
export const relativeLinks = (markdown) =>
  [...markdown.matchAll(/\]\(([^)\s]+)\)/g)]
    .map((m) => m[1])
    .filter((target) => !/^[a-z]+:/i.test(target));

/** Asserts that every relative link of the file at `path` names a file that exists, and a heading of it. */
export const assertLinksResolve = (path) => {
  for (const target of relativeLinks(read(path))) {
    const [file, anchor] = target.split("#");
    const resolved = file ? normalize(join(dirname(path), file)) : path;
    assert.ok(
      existsSync(join(root, resolved)),
      `${path} links ${target}, a file that does not exist`,
    );
    if (anchor && resolved.endsWith(".md")) {
      assert.ok(
        headings(read(resolved)).map(slug).includes(anchor),
        `${path} links ${target}, a heading that does not exist`,
      );
    }
  }
};

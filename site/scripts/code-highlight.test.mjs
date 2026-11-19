// The code section's syntax highlighting (NOC-360, the owner's point 15 of
// 2026-11-19): Astro's css-variables theme colours a token only through the
// --astro-code-* variables, so the section defines them from the brand's
// dark palette, in at least three colours, each readable on the code's
// ground (WCAG AA, 4.5:1). The install line is highlighted as a shell command.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const dist = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");
const html = readFileSync(join(dist, "index.html"), "utf8");

/** The code section's --astro-code-* variables that hold a hex colour. */
const palette = () =>
  Object.fromEntries(
    [...html.matchAll(/--astro-code-([a-z-]+):\s*(#[0-9a-f]{6})/gi)].map(
      (m) => [m[1], m[2].toLowerCase()],
    ),
  );

/** WCAG relative luminance of a #rrggbb colour. */
const luminance = (hex) => {
  const [r, g, b] = [1, 3, 5]
    .map((i) => Number.parseInt(hex.slice(i, i + 2), 16) / 255)
    .map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
const contrast = (a, b) => {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};

test("the tokens take at least three colours of the brand's palette", () => {
  const colours = palette();
  for (const token of [
    "background",
    "foreground",
    "token-keyword",
    "token-string",
    "token-function",
    "token-comment",
    "token-punctuation",
  ])
    assert.ok(colours[token], `--astro-code-${token} is not a hex colour`);
  const tokens = Object.entries(colours).filter(([name]) =>
    name.startsWith("token-"),
  );
  assert.ok(
    new Set(tokens.map(([, hex]) => hex)).size >= 3,
    "the code is flat",
  );
  assert.ok(
    !tokens.some(([, hex]) => hex === "#ff3d8b"),
    "magenta is the drawing's alone",
  );
});

test("every token colour reads on the code's ground at 4.5:1 or more", () => {
  const colours = palette();
  for (const [name, hex] of Object.entries(colours)) {
    if (name === "background") continue;
    const ratio = contrast(hex, colours.background);
    assert.ok(ratio >= 4.5, `${name} ${hex}: ${ratio.toFixed(2)}:1`);
  }
});

test("the install line is highlighted as a shell command", () => {
  const section = html.slice(html.indexOf('id="code"'));
  const start = section.indexOf('class="install');
  const install = section.slice(start, section.indexOf("</div>", start));
  assert.match(install, /astro-code[^>]*>[\s\S]*?var\(--astro-code-token-/);
});

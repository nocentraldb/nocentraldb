// Finds the assertions of a test file for the test-change check (NOC-508),
// each as its text and its skeleton: the text with every literal blanked, so
// that two assertions that differ only in an expected value share it.

const lineAt = (text, index) => text.slice(0, index).split("\n").length;

const OPEN = "([{";
const CLOSE = ")]}";

/** The index just past the bracket group that opens at `start`. */
function pastGroup(text, start) {
  let depth = 0;
  for (let i = start; i < text.length; i += 1) {
    if (OPEN.includes(text[i])) depth += 1;
    else if (CLOSE.includes(text[i])) {
      depth -= 1;
      if (depth === 0) return i + 1;
    }
  }
  return text.length;
}

/**
 * The index where the TypeScript statement that starts at `start` ends: a
 * `;` or `,` outside brackets, a bracket that closes one opened before it,
 * or a line break after which the statement does not go on with `.`.
 */
function statementEnd(text, start) {
  let depth = 0;
  for (let i = start; i < text.length; i += 1) {
    const c = text[i];
    if (OPEN.includes(c)) depth += 1;
    else if (CLOSE.includes(c)) {
      depth -= 1;
      if (depth < 0) return i;
    } else if (depth === 0 && (c === ";" || c === ",")) return i;
    else if (depth === 0 && c === "\n" && !/^\s*\??\./.test(text.slice(i)))
      return i;
  }
  return text.length;
}

/** One line with single spaces, no space inside brackets, no trailing commas. */
export function normalize(text) {
  return text
    .replace(/\s+/g, " ")
    .replace(/,\s*([)\]}])/g, "$1")
    .replace(/([([])\s+/g, "$1")
    .replace(/\s+([)\]])/g, "$1")
    .replace(/\s+(\??\.)/g, "$1")
    .replace(/\s*,\s*/g, ", ")
    .trim();
}

const LITERAL = /\b\d[\w.]*|\b(?:true|false|null|undefined)\b/g;

/**
 * The skeleton of an assertion's masked code (lib/test-changes/mask.mjs),
 * whose strings are blank already: each number and boolean blanked too.
 */
export const skeleton = (code) => normalize(code).replace(LITERAL, "_");

const RUST_ASSERT = /(?<![\w$])\w*assert\w*!\s*[([{]/g;
const TS_ASSERT = /(?<![\w$.])(?:expect|assert)\b(?=\s*[.(])/g;

/**
 * The assertions of a masked file (`{ code, plain }`, lib/test-changes/
 * mask.mjs) that start inside one of `regions` ([from, to) index pairs; the
 * whole text when null), in kind "ts" or Rust.
 */
export function assertions({ code: text, plain }, kind, regions = null) {
  const inside = (i) => !regions || regions.some(([a, b]) => i >= a && i < b);
  const pattern = kind === "ts" ? TS_ASSERT : RUST_ASSERT;
  const found = [];
  for (const m of text.matchAll(pattern)) {
    if (!inside(m.index)) continue;
    const end =
      kind === "ts"
        ? statementEnd(text, m.index)
        : pastGroup(text, m.index + m[0].length - 1);
    found.push({
      text: normalize(plain.slice(m.index, end)),
      skeleton: skeleton(text.slice(m.index, end)),
      line: lineAt(text, m.index),
    });
  }
  return found;
}

/**
 * The test code of a Rust file that is not a test file: the body of each
 * function at `bodies` (indexes of the text after its name) and each item
 * under #[cfg(test)], as [from, to) index pairs.
 */
export function rustTestRegions(text, bodies) {
  const regions = [];
  const block = (from) => {
    const open = text.slice(from).search(/[{;]/);
    if (open === -1 || text[from + open] === ";") return;
    regions.push([from, pastGroup(text, from + open)]);
  };
  for (const m of text.matchAll(/#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]/g)) {
    block(m.index + m[0].length);
  }
  for (const from of bodies) block(from);
  return regions;
}

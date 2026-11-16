// The core's texts (NOC-329, T6): entries of kind `core` in
// texts/texts.json, named `core.<name>`, without a code; the SDK attaches
// the code when it passes a core text on, as the command line's `cli` texts
// have none. scripts/texts.mjs generates crates/ncdb-core/src/texts.rs from
// them, a constant per text or a function that fills its placeholders, and
// its gate reads the core's code with what is here, T5's rule for the
// command line:
//
// - any literal with two words in a row is a text written in code, except
//   on a line marked `// notation`; a comment is no text;
// - unlike the command line's rule, a word has a lower-case letter, so SQL's
//   keywords in capitals are no words; a panic's message (`expect`,
//   `unreachable!`, `panic!`, `assert!` and its kin) is no text, as it
//   describes a bug in the core; and an inline `#[cfg(test)]` module is a
//   test (pm, NOC-342);
// - every core text is used, as `texts::NAME` or `texts::name`.
//
// The gate reads all of crates/ncdb-core/src, outside its tests and the
// generated module.

import { readFileSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";
import { checkCode, cliUse, rustFiles, rustModule } from "./texts.mjs";

/** The core's generated module, and the code the gate reads, relative to the root. */
export const CORE_PATHS = {
  src: "crates/ncdb-core/src",
  module: "crates/ncdb-core/src/texts.rs",
};

/**
 * A word: two letters or more, one of them lower-case, so that SQL's
 * keywords in capitals are no words (pm, NOC-342).
 */
const WORD = String.raw`(?<![A-Za-z])(?=[A-Za-z]*[a-z])[A-Za-z]{2,}(?![A-Za-z])`;

/**
 * A literal with two words in a row: T5's rule, with the core's words, and
 * a literal continued over lines with a backslash read as one.
 */
const CORE_STRAYS = [
  [
    new RegExp(
      String.raw`"(?:[^"\\\n]|\\[\s\S])*?${WORD} ${WORD}(?:[^"\\\n]|\\[\s\S])*"`,
      "g",
    ),
    "a text",
  ],
];

/** `text` with every character but a newline a space, so lines keep their numbers. */
const blank = (text) => text.replace(/[^\n]/g, " ");

/** The index just after the bracket that closes the one at `open`, or the end. */
const closing = (source, open, [left, right]) => {
  let depth = 0;
  for (let i = open; i < source.length; i++) {
    if (source[i] === left) depth++;
    else if (source[i] === right && --depth === 0) return i + 1;
  }
  return source.length;
};

/** A call whose message describes a bug in the core: a panic's (pm, NOC-342). */
const PANIC = /\b(?:expect|unreachable!|panic!|assert(?:_eq|_ne)?!)\(/g;

/**
 * `source` as the gate reads it: an inline `#[cfg(test)]` module and the
 * string literals of a panic's call blanked out (pm, NOC-342).
 */
export const coreReadable = (source) => {
  let out = source;
  for (const match of source.matchAll(
    /^\s*#\[cfg\(test\)\]\s*\n\s*(?:pub(?:\([^)]*\))? )?mod \w+ \{/gm,
  )) {
    const open = match.index + match[0].length - 1;
    const end = closing(source, open, "{}");
    out =
      out.slice(0, match.index) +
      blank(out.slice(match.index, end)) +
      out.slice(end);
  }
  for (const match of out.matchAll(PANIC)) {
    const open = match.index + match[0].length - 1;
    const end = closing(out, open, "()");
    const call = out.slice(open, end).replace(/"(?:[^"\\]|\\.)*"/g, blank);
    out = out.slice(0, open) + call + out.slice(end);
  }
  return out;
};

/** The files of the core's code the gate reads, as [path, source], outside its tests. */
export const coreFiles = (root) =>
  rustFiles(join(root, CORE_PATHS.src)).map(([path, source]) => [
    relative(root, path),
    source,
  ]);

/** The concerns' check. Stub for the red step. */
export const checkConcerns = (_texts) => [];

/** The core's module, crates/ncdb-core/src/texts.rs, of the `core` texts. */
export const generateCore = (texts) =>
  rustModule(texts, "core", [
    "Every text the core words itself, from texts/texts.json (NOC-329, T6).",
    "Each text's comment says when it shows.",
  ]);

/** The problems of the core's code: stray texts, and core texts no code uses. */
export const checkCoreCode = (files, texts) =>
  checkCode(
    files.map(([path, source]) => [path, coreReadable(source)]),
    CORE_PATHS.module,
    texts.filter((t) => t.kind === "core"),
    { strays: CORE_STRAYS, use: cliUse },
  );

/** The gate's part for the core in the repository at `root`: writes the module when `write`, and answers the problems. */
export const coreGate = (root, texts, write) => {
  const problems = [];
  const generated = generateCore(texts);
  const module = join(root, CORE_PATHS.module);
  if (write) writeFileSync(module, generated);
  let committed = null;
  try {
    committed = readFileSync(module, "utf8");
  } catch {}
  if (committed !== generated)
    problems.push(
      `${CORE_PATHS.module} differs from texts/texts.json: run node scripts/texts.mjs --write`,
    );
  problems.push(...checkCoreCode(coreFiles(root), texts));
  return problems;
};

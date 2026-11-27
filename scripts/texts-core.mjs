// The core's texts (NOC-329, T6): entries of kind `core` in
// texts/core/, named `core.<name>`, without a code; the SDK attaches
// the code when it passes a core text on, as the command line's `cli` texts
// have none. scripts/texts.mjs generates crates/ncdb-core/src/texts/ from
// them, one file per concern (`CONCERNS`), which its mod.rs re-exports, and
// in it a constant per text or a function that fills its placeholders; and
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

import {
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join, relative } from "node:path";
import { checkCode, cliUse, rustFiles, rustModule } from "./texts.mjs";

/** The core's generated module, and the code the gate reads, relative to the root. */
export const CORE_PATHS = {
  src: "crates/ncdb-core/src",
  module: "crates/ncdb-core/src/texts",
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

/**
 * The files of the core's module, one per concern, each with the first
 * words of the names it holds, so that no file grows past the repository's
 * 700-line limit (NOC-342).
 */
export const CONCERNS = {
  account: ["The texts of pairing and recovery.", ["pairing", "recovery"]],
  change: [
    "The texts of a change: its frame, its seal, its clock, its log and its control body.",
    ["append", "body", "change", "clock", "envelope", "open", "receive"],
  ],
  lock: [
    "The texts of the model's lock: reading it, and the changes it refuses.",
    ["lock", "name", "refusal"],
  ],
  value: [
    "The texts of a field's value, and of the plugins that define one.",
    ["anchor", "link", "plugin", "value"],
  ],
  view: [
    "The texts of the local view, and what it could not read.",
    ["unreadable", "view"],
  ],
};

/** The concern of a core text: the file whose words hold its name's first word. */
const concernOf = (entry) => {
  const word = entry.id.slice("core.".length).match(/^[a-z]+/)?.[0];
  return Object.keys(CONCERNS).find((c) => CONCERNS[c][1].includes(word));
};

/** The problems of the core texts' names: each begins with a concern's word. */
export const checkConcerns = (texts) =>
  texts
    .filter((t) => t.kind === "core" && !concernOf(t))
    .map(
      (t) =>
        `${t.id}: its name begins with no concern's word; add the word to CONCERNS in scripts/texts-core.mjs`,
    );

/**
 * The core's module, crates/ncdb-core/src/texts/, as { file name: content }:
 * mod.rs, which re-exports every file, so code names a text `texts::NAME`,
 * and one file per concern that has texts.
 */
export const generateCore = (texts) => {
  const core = texts.filter((t) => t.kind === "core");
  const concerns = Object.keys(CONCERNS).filter((c) =>
    core.some((t) => concernOf(t) === c),
  );
  const files = {
    "mod.rs": [
      "// Generated by scripts/texts.mjs from texts/: do not edit.",
      "// Change a text there, then run `node scripts/texts.mjs --write`.",
      "",
      "//! Every text the core words itself, from texts/ (NOC-329, T6),",
      "//! one file per concern. Each text's comment says when it shows.",
      "",
      ...concerns.flatMap((c) => [
        `/// ${CONCERNS[c][0]}`,
        `mod ${c};`,
        `pub(crate) use ${c}::*;`,
      ]),
    ]
      .join("\n")
      .concat("\n"),
  };
  for (const c of concerns)
    files[`${c}.rs`] = rustModule(
      core.filter((t) => concernOf(t) === c),
      "core",
      [CONCERNS[c][0]],
    );
  return files;
};

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
  problems.push(...checkConcerns(texts));
  if (problems.length > 0) return problems;
  const generated = generateCore(texts);
  const dir = join(root, CORE_PATHS.module);
  if (write) {
    mkdirSync(dir, { recursive: true });
    for (const name of readdirSync(dir))
      if (!(name in generated)) rmSync(join(dir, name));
    for (const [name, content] of Object.entries(generated))
      writeFileSync(join(dir, name), content);
  }
  let present = [];
  try {
    present = readdirSync(dir);
  } catch {}
  for (const name of [
    ...new Set([...present, ...Object.keys(generated)]),
  ].sort()) {
    let committed = null;
    try {
      committed = readFileSync(join(dir, name), "utf8");
    } catch {}
    if (committed !== (generated[name] ?? null))
      problems.push(
        `${CORE_PATHS.module}/${name} differs from texts/: run node scripts/texts.mjs --write`,
      );
  }
  problems.push(...checkCoreCode(coreFiles(root), texts));
  return problems;
};

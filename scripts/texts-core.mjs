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
// - every core text is used, as `texts::NAME` or `texts::name`.
//
// The gate reads all of crates/ncdb-core/src, outside its tests and the
// generated module.

import { readFileSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";
import {
  CLI_STRAYS,
  checkCode,
  cliUse,
  rustFiles,
  rustModule,
} from "./texts.mjs";

/** The core's generated module, and the code the gate reads, relative to the root. */
export const CORE_PATHS = {
  src: "crates/ncdb-core/src",
  module: "crates/ncdb-core/src/texts.rs",
};

/**
 * A literal with two words in a row: T5's rule for the command line. A
 * function, as texts.mjs imports this module before it defines CLI_STRAYS.
 */
const coreStrays = () => CLI_STRAYS.filter(([, what]) => what === "a text");

/** The files of the core's code the gate reads, as [path, source], outside its tests. */
export const coreFiles = (root) =>
  rustFiles(join(root, CORE_PATHS.src)).map(([path, source]) => [
    relative(root, path),
    source,
  ]);

/** The core's module, crates/ncdb-core/src/texts.rs, of the `core` texts. */
export const generateCore = (texts) =>
  rustModule(texts, "core", [
    "Every text the core words itself, from texts/texts.json (NOC-329, T6).",
    "Each text's comment says when it shows.",
  ]);

/** The problems of the core's code: stray texts, and core texts no code uses. */
export const checkCoreCode = (files, texts) =>
  checkCode(
    files,
    CORE_PATHS.module,
    texts.filter((t) => t.kind === "core"),
    { strays: coreStrays(), use: cliUse },
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

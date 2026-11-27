// The size of the generated text modules (NOC-541). `repo:file-size` fails
// on any code file over the limit factory/CONFIG.md sets, the generated
// modules included. A module grows with every new text, so the texts gate
// checks what it generates against the same limit first, and names the
// generator's fix: a kind's number of shards in `SHARDS`
// (scripts/texts.mjs), or a concern of the core in `CONCERNS`
// (scripts/texts-core.mjs).

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { generate, generateCli, PATHS } from "./texts.mjs";
import { CORE_PATHS, generateCore } from "./texts-core.mjs";
import { generateTs, TS_PATHS } from "./texts-ts.mjs";

/** Every generated text file of the four modules, as { path from the root: content }. */
export const generatedModules = (texts) => {
  const files = {};
  for (const [name, content] of Object.entries(generate(texts)))
    files[`${PATHS.module}/${name}`] = content;
  files[PATHS.cliModule] = generateCli(texts);
  files[TS_PATHS.module] = generateTs(texts);
  for (const [name, content] of Object.entries(generateCore(texts)))
    files[`${CORE_PATHS.module}/${name}`] = content;
  return files;
};

/** Lines as `repo:file-size` counts them. */
const countLines = (text) => {
  const breaks = text.split("\n").length - 1;
  return text === "" || text.endsWith("\n") ? breaks : breaks + 1;
};

/**
 * The problems of generated `files` longer than `limit` lines, so the gate
 * names the generator's fix before `repo:file-size` fails on the file.
 */
export const tooLong = (files, limit) =>
  Object.entries(files)
    .map(([path, content]) => [path, countLines(content)])
    .filter(([, lines]) => lines > limit)
    .map(
      ([path, lines]) =>
        `${path} would have ${lines} lines; the limit is ${limit} (factory/CONFIG.md). Split it: raise its kind's number in SHARDS in scripts/texts.mjs, or add a concern to CONCERNS in scripts/texts-core.mjs`,
    );

/** The limit for a file, from factory/CONFIG.md, as `repo:file-size` reads it. */
export const fileLimit = (root) =>
  Number(
    readFileSync(join(root, "factory/CONFIG.md"), "utf8").match(
      /^\|\s*Largest code or test file\s*\|\s*(\d+) lines\b/m,
    )[1],
  );

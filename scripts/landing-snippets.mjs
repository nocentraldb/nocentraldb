// Type-checks the TypeScript code of the landing page's "Get started"
// section, site/src/components/code-ts.md, against the SDK's source (NOC-371),
// as the doc tests of nocentraldb compile its Rust code (code-rust.md).
//
// The file holds one section per step, `## <step id>`, with one ```ts block.
// Each block is a module of its own: the model step's is model.ts, every
// other step's is <step id>.ts beside it. Lines that start with "# " (or are
// a bare "#") are hidden, as rustdoc's are: they compile, and the page leaves
// them out, so a snippet stays as short as it is shown. A shown import of
// "./model" is the model, and a JSON import, such as the lock, is declared
// `unknown`, as in checkPage (doc-snippets.mjs). Errors are given as
// `<file>:<line>:<column>`.
//
//   node scripts/landing-snippets.mjs [code-ts.md]
import { readFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { typeCheck } from "./doc-snippets.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
export const PAGE = join(ROOT, "site", "src", "components", "code-ts.md");
const HIDDEN = /^#( |$)/;
const STEP = /^[a-z][a-z0-9-]*$/;

/**
 * The steps of `markdown`, each { id, line, code, shown }: `line` the 1-based
 * line of the block's first code line, `code` every line with the hidden
 * mark taken off, `shown` the lines the page shows.
 */
export const sections = (markdown) => {
  const found = [];
  const lines = markdown.split("\n");
  let id;
  let open;
  for (const [i, text] of lines.entries()) {
    const heading = text.match(/^## (.*)$/);
    if (open === undefined && heading) {
      id = heading[1].trim();
      if (!STEP.test(id)) throw new Error(`line ${i + 1}: ${id} is no step id`);
      if (found.some((s) => s.id === id)) {
        throw new Error(`line ${i + 1}: ${id} is named twice`);
      }
    } else if (open === undefined && /^```ts\s*$/.test(text)) {
      if (id === undefined || found.some((s) => s.id === id)) {
        throw new Error(`line ${i + 1}: a ts block outside a step of its own`);
      }
      open = { id, line: i + 2, lines: [] };
    } else if (open !== undefined && text === "```") {
      found.push({
        id: open.id,
        line: open.line,
        code: open.lines.map((l) => l.replace(HIDDEN, "")).join("\n"),
        shown: open.lines.filter((l) => !HIDDEN.test(l)).join("\n"),
      });
      open = undefined;
    } else if (open !== undefined) {
      open.lines.push(text);
    }
  }
  if (open !== undefined) {
    throw new Error(`line ${open.line - 1}: a ts block is never closed`);
  }
  return found;
};

/** `code` with "./model" resolved to model.ts and a JSON import declared, line for line. */
const module = (code) =>
  code
    .split("\n")
    .map((text) => {
      const json = text.match(/^import\s+(\w+)\s+from\s+"[^"]+\.json";$/);
      if (json) return `declare const ${json[1]}: unknown;`;
      return text.replace(/from "\.\/model";$/, 'from "./model.ts";');
    })
    .join("\n");

/** Type-checks the steps of the page at `path`: { ok, output, count }. */
export const check = (path = PAGE) => {
  const steps = sections(readFileSync(path, "utf8"));
  const shown = relative(process.cwd(), path) || path;
  if (steps.length === 0) {
    return { ok: false, output: `${shown}: no ts block to check`, count: 0 };
  }
  const file = (s) => `${s.id}.ts`;
  const result = typeCheck(
    Object.fromEntries(steps.map((s) => [file(s), module(s.code)])),
    (name, line, column) => {
      const at = steps.find((s) => file(s) === name);
      return at && `${shown}:${at.line + line - 1}:${column}`;
    },
  );
  return { ...result, count: steps.length };
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const { ok, output, count } = check(process.argv[2]);
  if (output.trim() !== "") console.error(output.trimEnd());
  if (!ok) process.exit(1);
  console.log(`landing-snippets: ${count} snippets type-check`);
}

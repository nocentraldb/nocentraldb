// Type-checks the TypeScript snippets of the browser getting-started guide,
// docs/getting-started-browser.md, against the SDK's source (NOC-322), the
// way the doc tests of `nocentraldb` compile the Rust guide's snippets: the
// guide cannot drift from the SDK's public API.
//
// Every ```ts block of the guide is one module of an app laid out as
// apps/tasks-ts is. A block whose info string names a file, such as
// ```ts model.ts, is written under that path; every other block is a module
// of its own in src/, as the app's src/app.ts would be, so it imports the
// model as "../model.ts". `@nocentraldb/sdk` resolves to packages/sdk/src/index.ts, as in
// the type check of apps/tasks-ts, and tsc runs with tsconfig.base.json.
// Errors are reported at their line in the guide.
//
//   node scripts/doc-snippets.mjs [guide.md]
import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
export const GUIDE = join(ROOT, "docs", "getting-started-browser.md");
const TSC = join(ROOT, "node_modules", ".bin", "tsc");
const FILE = /^(?:[a-z0-9][a-z0-9-]*\/)*[a-z0-9][a-z0-9-]*\.ts$/;

/** The ```ts blocks of `markdown`: { file, line, code }, `line` the 1-based line of the block's first code line. */
export const snippets = (markdown) => {
  const found = [];
  const lines = markdown.split("\n");
  let open;
  for (const [i, text] of lines.entries()) {
    if (open === undefined) {
      const fence = text.match(/^```ts(?:\s+(\S+))?\s*$/);
      if (fence) open = { name: fence[1], line: i + 2, code: [] };
    } else if (text === "```") {
      const file = open.name ?? `src/snippet-${found.length + 1}.ts`;
      if (!FILE.test(file)) {
        throw new Error(`line ${open.line - 1}: ${file} is not a file name`);
      }
      if (found.some((s) => s.file === file)) {
        throw new Error(`line ${open.line - 1}: ${file} is named twice`);
      }
      found.push({ file, line: open.line, code: open.code.join("\n") });
      open = undefined;
    } else {
      open.code.push(text);
    }
  }
  if (open !== undefined) {
    throw new Error(`line ${open.line - 1}: a ts block is never closed`);
  }
  return found;
};

/**
 * Type-checks `files` ({ path: code }) as one app against the SDK's source:
 * { ok, output }, each error's `file(line,column)` handed to `place`, which
 * answers where it is in the page, or undefined to keep it.
 */
const typeCheck = (files, place) => {
  const dir = mkdtempSync(join(tmpdir(), "doc-snippets-"));
  try {
    writeFileSync(join(dir, "package.json"), '{ "type": "module" }\n');
    writeFileSync(
      join(dir, "tsconfig.json"),
      JSON.stringify({
        extends: join(ROOT, "tsconfig.base.json"),
        compilerOptions: {
          paths: {
            "@nocentraldb/sdk": [
              join(ROOT, "packages", "sdk", "src", "index.ts"),
            ],
          },
        },
        include: ["**/*.ts"],
      }),
    );
    for (const [file, code] of Object.entries(files)) {
      mkdirSync(dirname(join(dir, file)), { recursive: true });
      writeFileSync(join(dir, file), `${code}\n`);
    }
    const result = spawnSync(TSC, ["-p", dir, "--pretty", "false"], {
      cwd: dir,
      encoding: "utf8",
    });
    if (result.error) return { ok: false, output: String(result.error) };
    // tsc names a file and a position: `src/snippet-2.ts(3,5): error TS...`.
    const output = `${result.stdout}${result.stderr}`.replace(
      /^([a-z0-9/-]+\.ts)\((\d+),(\d+)\)/gm,
      (whole, file, line, column) => place(file, Number(line), column) ?? whole,
    );
    return { ok: result.status === 0, output };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
};

/** Type-checks the snippets of the guide at `path`: { ok, output }, errors given as `<guide>:<line>:<column>`. */
export const check = (path = GUIDE) => {
  const blocks = snippets(readFileSync(path, "utf8"));
  if (blocks.length === 0) {
    return { ok: false, output: `${path}: no ts snippet to check` };
  }
  const shown = relative(process.cwd(), path) || path;
  const result = typeCheck(
    Object.fromEntries(blocks.map((b) => [b.file, b.code])),
    (file, line, column) => {
      const block = blocks.find((b) => b.file === file);
      return block && `${shown}:${block.line + line - 1}:${column}`;
    },
  );
  return { ...result, count: blocks.length };
};

const IMPORT = /^import\s.*\sfrom\s+"([^"]+)";$/;
const IMPLIED = [
  'import { nc } from "@nocentraldb/sdk";',
  'import { model } from "../model.ts";',
];

/**
 * Type-checks the examples of a page whose ts blocks read as one app, as the
 * README's and Getting started's do (NOC-361): the block with `nc.model(` is
 * model.ts, and every other block continues the one before it, in one
 * module that imports `nc` and `model`. Imports are hoisted and each kept
 * once; `./model` is the model, and a JSON import, such as the lock, is
 * declared `unknown`. `declare` names what the page leaves to the reader,
 * such as `declare function render(page: unknown): void;`. Errors are given
 * as `<page>:<line>:<column>`.
 */
export const checkPage = (path, { declare = [] } = {}) => {
  const blocks = snippets(readFileSync(path, "utf8"));
  const shown = relative(process.cwd(), path) || path;
  const model = blocks.find((b) => b.code.includes("nc.model("));
  if (!model) {
    return { ok: false, output: `${shown}: no ts block with the model` };
  }
  const head = new Set(IMPLIED);
  const declared = [];
  const body = [];
  for (const block of blocks.filter((b) => b !== model)) {
    for (const [i, text] of block.code.split("\n").entries()) {
      const from = text.match(IMPORT)?.[1];
      if (from?.endsWith(".json")) {
        const name = text.match(/^import\s+(\w+)\s/)?.[1];
        declared.push(`declare const ${name}: unknown;`);
        body.push({ text: "", line: block.line + i });
      } else if (from !== undefined) {
        head.add(from === "./model" ? IMPLIED[1] : text);
        body.push({ text: "", line: block.line + i });
      } else {
        body.push({ text, line: block.line + i });
      }
    }
  }
  const prelude = [...head, ...new Set(declared), ...declare];
  const result = typeCheck(
    {
      "model.ts": model.code,
      "src/page.ts": [...prelude, ...body.map((b) => b.text)].join("\n"),
    },
    (file, line, column) => {
      if (file === "model.ts")
        return `${shown}:${model.line + line - 1}:${column}`;
      const at = body[line - prelude.length - 1];
      return at && `${shown}:${at.line}:${column}`;
    },
  );
  return { ...result, count: blocks.length };
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const { ok, output, count } = check(process.argv[2]);
  if (output.trim() !== "") console.error(output.trimEnd());
  if (!ok) process.exit(1);
  console.log(`doc-snippets: ${count} snippets type-check`);
}

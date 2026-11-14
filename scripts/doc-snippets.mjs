// Type-checks the TypeScript snippets of the browser getting-started guide,
// docs/getting-started-browser.md, against the SDK's source (NOC-322), the
// way the doc tests of `nocentraldb` compile the Rust guide's snippets: the
// guide cannot drift from the SDK's public API.
//
// Every ```ts block of the guide is one module. A block whose info string
// names a file, such as ```ts model.ts, is written under that name, so the
// other snippets import it as "./model.ts"; every other block is a module of
// its own. `@nocentraldb/sdk` resolves to packages/sdk/src/index.ts, as in
// the type check of apps/tasks-ts, and tsc runs with tsconfig.base.json.
// Errors are reported at their line in the guide.
//
//   node scripts/doc-snippets.mjs [guide.md]
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
export const GUIDE = join(ROOT, "docs", "getting-started-browser.md");
const TSC = join(ROOT, "node_modules", ".bin", "tsc");
const FILE = /^[a-z0-9][a-z0-9-]*\.ts$/;

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
      const file = open.name ?? `snippet-${found.length + 1}.ts`;
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

/** Type-checks the snippets of the guide at `path`: { ok, output }, errors given as `<guide>:<line>:<column>`. */
export const check = (path = GUIDE) => {
  const blocks = snippets(readFileSync(path, "utf8"));
  if (blocks.length === 0) {
    return { ok: false, output: `${path}: no ts snippet to check` };
  }
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
        include: ["*.ts"],
      }),
    );
    for (const block of blocks) {
      writeFileSync(join(dir, block.file), `${block.code}\n`);
    }
    const result = spawnSync(TSC, ["-p", dir, "--pretty", "false"], {
      cwd: dir,
      encoding: "utf8",
    });
    const shown = relative(process.cwd(), path) || path;
    // tsc names a file and a position: `model.ts(3,5): error TS...`.
    const output = `${result.stdout}${result.stderr}`.replace(
      /^([a-z0-9-]+\.ts)\((\d+),(\d+)\)/gm,
      (whole, file, line, column) => {
        const block = blocks.find((b) => b.file === file);
        if (!block) return whole;
        return `${shown}:${block.line + Number(line) - 1}:${column}`;
      },
    );
    if (result.error) return { ok: false, output: String(result.error) };
    return { ok: result.status === 0, output, count: blocks.length };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const { ok, output, count } = check(process.argv[2]);
  if (output.trim() !== "") console.error(output.trimEnd());
  if (!ok) process.exit(1);
  console.log(`doc-snippets: ${count} snippets type-check`);
}

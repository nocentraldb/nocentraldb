// Writes the probe page as a self-contained static folder: the page
// (page/) and the SQLite build it imports (sqlite/), with the same layout
// server.mjs serves. Any static HTTPS host serves it; it needs `.wasm` served
// as application/wasm, and no other headers. The manual checks on a phone
// use it (docs/browser-storage-checklist.md).
//
//   node scripts/browser-storage/build.mjs <out dir>
import { copyFile, mkdir } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { PAGE_DIR, SQLITE_DIR } from "./server.mjs";

const FILES = [
  [join(PAGE_DIR, "index.html"), "index.html"],
  [join(PAGE_DIR, "probe.js"), "probe.js"],
  [join(PAGE_DIR, "worker.js"), "worker.js"],
  [join(SQLITE_DIR, "dist", "index.mjs"), "sqlite/index.mjs"],
  [join(SQLITE_DIR, "dist", "sqlite3.wasm"), "sqlite/sqlite3.wasm"],
];

/** Copies the page and the SQLite build into `out`; resolves to the file list. */
export const build = async (out) => {
  await mkdir(join(out, "sqlite"), { recursive: true });
  for (const [from, to] of FILES) await copyFile(from, join(out, to));
  return FILES.map(([, to]) => to);
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const out = process.argv[2];
  if (!out) {
    console.error("usage: node scripts/browser-storage/build.mjs <out dir>");
    process.exit(2);
  }
  for (const file of await build(out)) console.log(join(out, file));
}

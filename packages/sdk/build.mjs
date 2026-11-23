// Builds @nocentraldb/sdk into dist/ (Carsten, 2026-11-12, NOC-274, point 3):
//
// 1. the core: the crate nocentraldb-wasm for wasm32, in release;
// 2. its JavaScript glue: wasm-bindgen, at the version the root task
//    `wasm-runner` installs into target/tools, into dist/wasm/;
// 3. the TypeScript: tsc with tsconfig.build.json, into dist/;
// 4. the core of the `ncdb` bin (dist/ncdb.js): build-ncdb.mjs, into
//    dist/ncdb/, right after step 2 (NOC-415; Carsten, 2026-11-23,
//    NOC-443, A).
//
// The worker (dist/worker.js) finds dist/wasm/ with new URL(...,
// import.meta.url), so an app's bundler copies what dist/ holds.
//
// Steps 2 and 3 write into a directory of their own under target/, and the
// build then moves each file into dist/ with a rename, which replaces a file
// at once, and removes what it no longer builds. A page that loads dist/
// while another build runs, as the sdk-browser e2e files do in parallel,
// reads every file whole: emptying dist/ first left it without its module
// for as long as the build took (NOC-336).
//
//   node packages/sdk/build.mjs           the whole package
//   node packages/sdk/build.mjs --ts-only only step 3, for the schema
//                                          functions `ncdb model build` reads
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readdirSync,
  renameSync,
  rmSync,
} from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const PACKAGE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(PACKAGE, "..", "..");
const DIST = join(PACKAGE, "dist");
/** Where this build writes before it moves its files into DIST. */
const BUILD = join(ROOT, "target", "sdk-dist", `build-${process.pid}`);
const BINDGEN = join(ROOT, "target", "tools", "bin", "wasm-bindgen");
const MODULE = join(
  ROOT,
  "target",
  "wasm32-unknown-unknown",
  "release",
  "nocentraldb_wasm.wasm",
);

const run = (command, args) => {
  const result = spawnSync(command, args, { cwd: ROOT, stdio: "inherit" });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed (${result.status})`);
  }
};

/** Every file under `dir`, as a path relative to it. */
const files = (dir) =>
  existsSync(dir)
    ? readdirSync(dir, { recursive: true, withFileTypes: true })
        .filter((entry) => entry.isFile())
        .map((entry) => relative(dir, join(entry.parentPath, entry.name)))
    : [];

/**
 * Moves every file of BUILD into DIST, each with a rename, and removes the
 * files of DIST that BUILD does not hold.
 */
const swap = () => {
  const built = new Set(files(BUILD));
  for (const file of built) {
    mkdirSync(dirname(join(DIST, file)), { recursive: true });
    renameSync(join(BUILD, file), join(DIST, file));
  }
  for (const file of files(DIST)) {
    if (!built.has(file)) rmSync(join(DIST, file));
  }
};

const tsOnly = process.argv.includes("--ts-only");

rmSync(BUILD, { recursive: true, force: true });
mkdirSync(BUILD, { recursive: true });
if (!tsOnly) {
  if (!existsSync(BINDGEN)) {
    throw new Error(`${BINDGEN} is missing: run \`moon run repo:wasm-runner\``);
  }
  run("cargo", [
    "build",
    "--locked",
    "--release",
    "-p",
    "nocentraldb-wasm",
    "--target",
    "wasm32-unknown-unknown",
  ]);
  run(BINDGEN, [
    "--target",
    "web",
    "--out-dir",
    join(BUILD, "wasm"),
    "--out-name",
    "nocentraldb_wasm",
    MODULE,
  ]);
  run("node", [join(PACKAGE, "build-ncdb.mjs"), join(BUILD, "ncdb")]);
}
run(join(ROOT, "node_modules", ".bin", "tsc"), [
  "-p",
  join(PACKAGE, "tsconfig.build.json"),
  "--outDir",
  BUILD,
]);
swap();
rmSync(BUILD, { recursive: true, force: true });

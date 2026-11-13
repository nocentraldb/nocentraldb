// Builds @nocentraldb/sdk into dist/ (Carsten, 2026-11-12, NOC-274, point 3):
//
// 1. the core: the crate nocentraldb-wasm for wasm32, in release;
// 2. its JavaScript glue: wasm-bindgen, at the version the root task
//    `wasm-runner` installs into target/tools, into dist/wasm/;
// 3. the TypeScript: tsc with tsconfig.build.json, into dist/.
//
// The worker (dist/worker.js) finds dist/wasm/ with new URL(...,
// import.meta.url), so an app's bundler copies what dist/ holds.
//
//   node packages/sdk/build.mjs           the whole package
//   node packages/sdk/build.mjs --ts-only only step 3, for the schema
//                                          functions `ncdb model build` reads
import { spawnSync } from "node:child_process";
import { existsSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const PACKAGE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(PACKAGE, "..", "..");
const DIST = join(PACKAGE, "dist");
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

const tsOnly = process.argv.includes("--ts-only");

rmSync(DIST, { recursive: true, force: true });
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
    join(DIST, "wasm"),
    "--out-name",
    "nocentraldb_wasm",
    MODULE,
  ]);
}
run(join(ROOT, "node_modules", ".bin", "tsc"), [
  "-p",
  join(PACKAGE, "tsconfig.build.json"),
]);

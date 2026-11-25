// A copy of the TypeScript SDK's package whose core traps on a marker value
// (NOC-499), for trap.e2e.test.mjs: the package's build output from
// packages/sdk/dist with its WebAssembly module built from
// crates/nocentraldb-wasm with the feature `test-trap`, in
// target/sdk-browser/trap-dist. The core is built in a target directory of
// its own, so it never replaces the module packages/sdk/build.mjs ships.
// The workflow browser-view.yml builds it on the runner after the package,
// and the test runs in Playwright's image, which has no Rust.
//
//   node scripts/sdk-browser/trap.mjs
import { spawnSync } from "node:child_process";
import { cpSync, existsSync, renameSync, rmSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { DIST, ROOT } from "./build.mjs";

/** The copy of the package with the trapping core. */
export const TRAP_DIST = join(ROOT, "target", "sdk-browser", "trap-dist");

/** The target directory of the trapping core. */
const TARGET = join(ROOT, "target", "sdk-trap");

const BINDGEN = join(ROOT, "target", "tools", "bin", "wasm-bindgen");

const run = (command, args) => {
  const result = spawnSync(command, args, { cwd: ROOT, stdio: "inherit" });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed (${result.status})`);
  }
};

/** Builds TRAP_DIST from DIST, which the package's build made before. */
export const buildTrap = () => {
  if (!existsSync(join(DIST, "worker.js"))) {
    throw new Error(`${DIST} is missing: run build.mjs first`);
  }
  run("cargo", [
    "build",
    "--locked",
    "--release",
    "-p",
    "nocentraldb-wasm",
    "--features",
    "test-trap",
    "--target",
    "wasm32-unknown-unknown",
    "--target-dir",
    TARGET,
  ]);
  // Built beside it and renamed over it, so a test that reads it sees it whole.
  const copy = `${TRAP_DIST}.${process.pid}`;
  rmSync(copy, { recursive: true, force: true });
  cpSync(DIST, copy, { recursive: true });
  run(BINDGEN, [
    "--target",
    "web",
    "--out-dir",
    join(copy, "wasm"),
    "--out-name",
    "nocentraldb_wasm",
    join(TARGET, "wasm32-unknown-unknown", "release", "nocentraldb_wasm.wasm"),
  ]);
  rmSync(TRAP_DIST, { recursive: true, force: true });
  renameSync(copy, TRAP_DIST);
};

/** The copy built before, as the workflow's image uses it; fails at once when it is missing. */
export const prebuiltTrap = () => {
  if (!existsSync(join(TRAP_DIST, "wasm", "nocentraldb_wasm_bg.wasm"))) {
    throw new Error(`${TRAP_DIST} is missing: run trap.mjs first`);
  }
};

if (process.argv[1] === fileURLToPath(import.meta.url)) buildTrap();

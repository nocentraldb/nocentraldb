// Builds the core of @nocentraldb/sdk's `ncdb` bin (NOC-415; Carsten,
// 2026-11-23, NOC-443, A): the crate ncdb-wasm for wasm32, and its
// JavaScript glue from wasm-bindgen, at the version the root task
// `wasm-runner` installs into target/tools, into the directory given.
// dist/ncdb.js loads it from dist/ncdb/.
//
// The build is optimized for size (LTO, opt-level "s", one codegen unit),
// which only the profile variables below set, so it uses a target directory
// of its own: the same variables in target/ would rebuild every other
// release build there. wasm-bindgen drops the name and producers sections;
// stripping in cargo instead would hide the module's wasm features from it.
//
//   node packages/sdk/build-ncdb.mjs <out-dir>
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const TARGET = join(ROOT, "target", "ncdb-wasm");
const BINDGEN = join(ROOT, "target", "tools", "bin", "wasm-bindgen");
const MODULE = join(
  TARGET,
  "wasm32-unknown-unknown",
  "release",
  "ncdb_wasm.wasm",
);

const out = process.argv[2];
if (!out) {
  process.stderr.write("usage: build-ncdb <out-dir>\n");
  process.exit(2);
}

const run = (command, args, env = {}) => {
  const result = spawnSync(command, args, {
    cwd: ROOT,
    stdio: "inherit",
    env: { ...process.env, ...env },
  });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed (${result.status})`);
  }
};

if (!existsSync(BINDGEN)) {
  throw new Error(`${BINDGEN} is missing: run \`moon run repo:wasm-runner\``);
}
run(
  "cargo",
  [
    "build",
    "--locked",
    "--release",
    "-p",
    "ncdb-wasm",
    "--target",
    "wasm32-unknown-unknown",
  ],
  {
    CARGO_TARGET_DIR: TARGET,
    CARGO_PROFILE_RELEASE_LTO: "true",
    CARGO_PROFILE_RELEASE_OPT_LEVEL: "s",
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS: "1",
  },
);
run(BINDGEN, [
  "--target",
  "web",
  "--remove-name-section",
  "--remove-producers-section",
  "--out-dir",
  out,
  "--out-name",
  "ncdb_wasm",
  MODULE,
]);

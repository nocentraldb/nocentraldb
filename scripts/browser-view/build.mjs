// Builds the harness of the browser view test (NOC-174) for the browser and
// runs it natively.
//
// - `buildHarness()` builds crates/ncdb-core/examples/browser_view.rs for
//   wasm32-unknown-unknown and turns it into an ES module with the pinned
//   wasm-bindgen that the root task `wasm-runner` installs into
//   target/tools, in target/browser-view/pkg, which server.mjs serves.
// - `runNative()` runs the same example natively and returns what it
//   prints: the view it read back and its SQLite release.
// - Run as a script, it does both and keeps what the native run printed in
//   target/browser-view/native.json. The workflow browser-view.yml builds
//   on the runner this way and runs the tests in Playwright's image, which
//   has no Rust (NOC-221); `prebuilt()` there reads the file instead of
//   building again.
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
export const PKG_DIR = join(ROOT, "target", "browser-view", "pkg");
export const NATIVE = join(ROOT, "target", "browser-view", "native.json");
const BINDGEN = join(ROOT, "target", "tools", "bin", "wasm-bindgen");
const EXAMPLE = ["-p", "ncdb-core", "--example", "browser_view"];

const run = (command, args) => {
  const result = spawnSync(command, args, {
    cwd: ROOT,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
  });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed (${result.status})`);
  }
  return result.stdout;
};

export const buildHarness = () => {
  if (!existsSync(BINDGEN)) {
    throw new Error(`${BINDGEN} is missing: run \`moon run repo:wasm-runner\``);
  }
  run("cargo", [
    "build",
    "--locked",
    ...EXAMPLE,
    "--target",
    "wasm32-unknown-unknown",
  ]);
  const wasm = join(
    ROOT,
    "target",
    "wasm32-unknown-unknown",
    "debug",
    "examples",
    "browser_view.wasm",
  );
  run(BINDGEN, ["--target", "web", "--out-dir", PKG_DIR, wasm]);
};

export const runNative = () =>
  JSON.parse(run("cargo", ["run", "--locked", "--quiet", ...EXAMPLE]));

/**
 * With BROWSER_VIEW_PREBUILT=1, what the native run printed when this file
 * ran as a script; the harness in PKG_DIR is that build's. Otherwise none.
 */
export const prebuilt = () => {
  if (process.env.BROWSER_VIEW_PREBUILT !== "1") return null;
  if (!existsSync(NATIVE) || !existsSync(PKG_DIR)) {
    throw new Error(
      `BROWSER_VIEW_PREBUILT=1, but ${NATIVE} or ${PKG_DIR} is missing: run \`node scripts/browser-view/build.mjs\``,
    );
  }
  return JSON.parse(readFileSync(NATIVE, "utf8"));
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  buildHarness();
  writeFileSync(NATIVE, JSON.stringify(runNative()));
  console.log(`harness: ${PKG_DIR}`);
  console.log(`native: ${NATIVE}`);
}

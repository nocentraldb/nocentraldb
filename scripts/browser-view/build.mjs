// Builds the harness of the browser view test (NOC-174) for the browser and
// runs it natively.
//
// - `buildHarness()` builds crates/ncdb-core/examples/browser_view.rs for
//   wasm32-unknown-unknown and turns it into an ES module with the pinned
//   wasm-bindgen that the root task `wasm-runner` installs into
//   target/tools, in target/browser-view/pkg, which server.mjs serves.
// - `runNative()` runs the same example natively and returns what it
//   prints: the view it read back and its SQLite release.
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
export const PKG_DIR = join(ROOT, "target", "browser-view", "pkg");
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

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  buildHarness();
  console.log(`harness: ${PKG_DIR}`);
}

// Builds the example app into dist/: by default first the SDK it imports
// (packages/sdk/build.mjs, which compiles the core to WebAssembly), then the
// app's TypeScript with tsc (tsconfig.build.json).
//
//   node apps/tasks-ts/build.mjs [--app-only]
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const APP = dirname(fileURLToPath(import.meta.url));
const ROOT = join(APP, "..", "..");
/** The SDK's build output, which the page imports. */
export const SDK = join(ROOT, "packages", "sdk", "dist");

const run = (command, args) => {
  const result = spawnSync(command, args, { cwd: ROOT, stdio: "inherit" });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed (${result.status})`);
  }
};

/** Builds the SDK unless `sdk` is false, then the app. */
export const build = ({ sdk = true } = {}) => {
  if (sdk) run(process.execPath, [join(ROOT, "packages", "sdk", "build.mjs")]);
  for (const file of [
    "index.js",
    "worker.js",
    "wasm/nocentraldb_wasm_bg.wasm",
  ]) {
    if (!existsSync(join(SDK, file))) {
      throw new Error(`${join(SDK, file)} is missing: build the SDK first`);
    }
  }
  run(join(ROOT, "node_modules", ".bin", "tsc"), [
    "-p",
    join(APP, "tsconfig.build.json"),
  ]);
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  build({ sdk: !process.argv.includes("--app-only") });
}

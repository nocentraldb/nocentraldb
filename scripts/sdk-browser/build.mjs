// Builds the TypeScript SDK the browser test imports (NOC-176 slice 1):
// packages/sdk's own build, which compiles the core to WebAssembly, runs
// wasm-bindgen and emits packages/sdk/dist/. The workflow browser-view.yml
// builds on the runner this way and runs the test in Playwright's image,
// which has no Rust (NOC-221).
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
/** The package's build output, which the page imports. */
export const DIST = join(ROOT, "packages", "sdk", "dist");
/** The lock the test opens with: the Rust SDK's test model. */
export const LOCK = join(
  ROOT,
  "crates",
  "nocentraldb",
  "tests",
  "model",
  "model.lock.json",
);

/** Builds packages/sdk/dist. */
export const build = () => {
  const result = spawnSync(
    process.execPath,
    [join(ROOT, "packages", "sdk", "build.mjs")],
    {
      cwd: ROOT,
      stdio: "inherit",
    },
  );
  if (result.status !== 0)
    throw new Error(`the SDK's build failed (${result.status})`);
};

if (process.argv[1] === fileURLToPath(import.meta.url)) build();

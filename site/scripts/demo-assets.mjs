// Puts the live demo's assets into site/public/demo (NOC-319), from which
// `astro build` copies them into dist/demo: the TypeScript SDK's build
// output (packages/sdk/dist: its JavaScript, its worker and its core in
// WebAssembly) into sdk/, and the lock of the example task list
// (apps/tasks-ts/model.lock.json), whose model the demo declares. The demo
// module itself, site/public/demo/demo.js, is in git; what this script
// copies is not (site/.gitignore). Build the SDK first:
// `node packages/sdk/build.mjs`.
//
//   node site/scripts/demo-assets.mjs
import { cpSync, existsSync, rmSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../..", import.meta.url));
const SDK = join(ROOT, "packages", "sdk", "dist");
const DEMO = join(ROOT, "site", "public", "demo");

/** The files the demo needs from the SDK's build output. */
const NEEDED = ["index.js", "worker.js", "wasm/nocentraldb_wasm_bg.wasm"];

export const copyDemoAssets = () => {
  for (const file of NEEDED) {
    if (!existsSync(join(SDK, file))) {
      throw new Error(
        `${join(SDK, file)} is missing: build the SDK first (node packages/sdk/build.mjs)`,
      );
    }
  }
  rmSync(join(DEMO, "sdk"), { recursive: true, force: true });
  // Only what runs in the browser: no type declarations.
  cpSync(SDK, join(DEMO, "sdk"), {
    recursive: true,
    filter: (source) => !source.endsWith(".d.ts"),
  });
  cpSync(
    join(ROOT, "apps", "tasks-ts", "model.lock.json"),
    join(DEMO, "model.lock.json"),
  );
};

if (process.argv[1] === fileURLToPath(import.meta.url)) copyDemoAssets();

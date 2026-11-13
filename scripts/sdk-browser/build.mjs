// Builds the TypeScript SDK the browser test imports (NOC-176 slice 1):
// packages/sdk's own build, which compiles the core to WebAssembly, runs
// wasm-bindgen and emits packages/sdk/dist/. Slice 4 syncs with the local
// relay server, which it builds into target/sdk-browser/ncdb-relay. The workflow browser-view.yml
// builds on the runner this way and runs the test in Playwright's image,
// which has no Rust (NOC-221).
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
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

/** The local relay server the sync tests start. */
export const RELAY = join(ROOT, "target", "sdk-browser", "ncdb-relay");

/** Builds the local relay server into RELAY. */
const buildRelay = () => {
  const result = spawnSync(
    "cargo",
    ["build", "--locked", "-p", "ncdb-relay-server", "--bin", "ncdb-relay"],
    { cwd: ROOT, stdio: ["ignore", "inherit", "inherit"] },
  );
  if (result.status !== 0) {
    throw new Error(`cargo build of ncdb-relay failed (${result.status})`);
  }
  mkdirSync(dirname(RELAY), { recursive: true });
  copyFileSync(join(ROOT, "target", "debug", "ncdb-relay"), RELAY);
};

/** Builds packages/sdk/dist and the local relay server. */
export const build = () => {
  buildRelay();
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

/** The package built before, as the workflow's image uses it; fails at once when it is missing. */
export const prebuilt = () => {
  for (const file of [
    "index.js",
    "worker.js",
    "wasm/nocentraldb_wasm_bg.wasm",
  ]) {
    if (!existsSync(join(DIST, file))) {
      throw new Error(`${join(DIST, file)} is missing: run build.mjs first`);
    }
  }
  if (!existsSync(RELAY)) {
    throw new Error(`${RELAY} is missing: run build.mjs first`);
  }
};

if (process.argv[1] === fileURLToPath(import.meta.url)) build();

// A copy of the TypeScript SDK's package whose core holds rich text
// (NOC-461), for richText.e2e.test.mjs: the package's build output from
// packages/sdk/dist with its WebAssembly module built from
// crates/nocentraldb-wasm with the compiler cfg `ncdb_rich_text`, in
// target/sdk-browser/rich-text-dist. The core is built in the target
// directory the crates' rich-text tasks use, so it never replaces the
// module packages/sdk/build.mjs ships, which refuses one with rich text
// (packages/sdk/richText.mjs). The workflow browser-view.yml builds it on
// the runner after the package, as it builds the trapping copy.
//
//   node scripts/sdk-browser/richTextCopy.mjs
import { spawnSync } from "node:child_process";
import { cpSync, existsSync, renameSync, rmSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { DIST, ROOT } from "./build.mjs";

/** The copy of the package with a rich-text core. */
export const RICH_TEXT_DIST = join(
  ROOT,
  "target",
  "sdk-browser",
  "rich-text-dist",
);

/** The target directory of the rich-text core, the crates' rich-text tasks' own. */
const TARGET = join(ROOT, "target", "rich-text");

const BINDGEN = join(ROOT, "target", "tools", "bin", "wasm-bindgen");

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

/** Builds RICH_TEXT_DIST from DIST, which the package's build made before. */
export const buildRichText = () => {
  if (!existsSync(join(DIST, "worker.js"))) {
    throw new Error(`${DIST} is missing: run build.mjs first`);
  }
  run(
    "cargo",
    [
      "build",
      "--locked",
      "--release",
      "-p",
      "nocentraldb-wasm",
      "--target",
      "wasm32-unknown-unknown",
      "--target-dir",
      TARGET,
    ],
    { RUSTFLAGS: "--cfg ncdb_rich_text" },
  );
  // Built beside it and renamed over it, so a test that reads it sees it whole.
  const copy = `${RICH_TEXT_DIST}.${process.pid}`;
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
  rmSync(RICH_TEXT_DIST, { recursive: true, force: true });
  renameSync(copy, RICH_TEXT_DIST);
};

/** The copy built before, as the workflow's image uses it; fails at once when it is missing. */
export const prebuiltRichText = () => {
  if (!existsSync(join(RICH_TEXT_DIST, "wasm", "nocentraldb_wasm_bg.wasm"))) {
    throw new Error(`${RICH_TEXT_DIST} is missing: run richTextCopy.mjs first`);
  }
};

if (process.argv[1] === fileURLToPath(import.meta.url)) buildRichText();

// Builds the site with its live demo (NOC-319): the TypeScript SDK first,
// its core in WebAssembly included (packages/sdk/build.mjs), unless
// SDK_BROWSER_PREBUILT=1 says it is built; then the demo's assets into
// site/public/demo (site/scripts/demo-assets.mjs); then the site.
// SITE_DEMO_PREBUILT=1 skips all of it: the site is built already.
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../..", import.meta.url));

const run = (command, args) => {
  const result = spawnSync(command, args, { cwd: ROOT, stdio: "inherit" });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed (${result.status})`);
  }
};

export const build = () => {
  // In CI the runner built the site, and the tests run in Playwright's
  // image on it (browser-view.yml).
  if (process.env.SITE_DEMO_PREBUILT === "1") return;
  if (process.env.SDK_BROWSER_PREBUILT !== "1") {
    run(process.execPath, ["packages/sdk/build.mjs"]);
  }
  run(process.execPath, ["site/scripts/demo-assets.mjs"]);
  run("pnpm", ["--dir", "site", "build"]);
};

if (process.argv[1] === fileURLToPath(import.meta.url)) build();

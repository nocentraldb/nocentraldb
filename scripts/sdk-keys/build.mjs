// Builds the Rust SDK's wasm32 library tests for the browser key test
// (NOC-175 slice 2), and keeps the test module at a fixed path,
// target/sdk-keys/nocentraldb-lib-tests.wasm, which keys.e2e.test.mjs
// serves through the pinned wasm-bindgen-test-runner that the root task
// `wasm-runner` installs into target/tools.
//
// Run as a script, it builds. The workflow browser-view.yml builds on the
// runner this way and runs the test in Playwright's image, which has no
// Rust (NOC-221); `prebuilt()` there uses the module built before.
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
export const OUT = join(
  ROOT,
  "target",
  "sdk-keys",
  "nocentraldb-lib-tests.wasm",
);
export const RUNNER = join(
  ROOT,
  "target",
  "tools",
  "bin",
  "wasm-bindgen-test-runner",
);

/** Builds the SDK's wasm32 library tests and returns the module's path. */
export const buildTests = () => {
  if (!existsSync(RUNNER)) {
    throw new Error(`${RUNNER} is missing: run \`moon run repo:wasm-runner\``);
  }
  const result = spawnSync(
    "cargo",
    [
      "test",
      "--locked",
      "-p",
      "nocentraldb",
      "--lib",
      "--target",
      "wasm32-unknown-unknown",
      "--no-run",
      "--message-format=json",
    ],
    {
      cwd: ROOT,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "inherit"],
      maxBuffer: 1 << 28,
    },
  );
  if (result.status !== 0) {
    throw new Error(`cargo test --no-run failed (${result.status})`);
  }
  const module = result.stdout
    .split("\n")
    .filter((line) => line.startsWith("{"))
    .map((line) => JSON.parse(line))
    .find(
      (message) =>
        message.reason === "compiler-artifact" &&
        message.target?.name === "nocentraldb" &&
        message.profile?.test === true &&
        message.executable?.endsWith(".wasm"),
    )?.executable;
  if (!module)
    throw new Error("cargo built no test module of the SDK's library");
  mkdirSync(dirname(OUT), { recursive: true });
  copyFileSync(module, OUT);
  return OUT;
};

/** The module built before, with SDK_KEYS_PREBUILT=1, or null. */
export const prebuilt = () => {
  if (process.env.SDK_KEYS_PREBUILT !== "1") return null;
  if (!existsSync(OUT))
    throw new Error(`${OUT} is missing: run build.mjs first`);
  return OUT;
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  console.log(buildTests());
}

// Builds the Rust SDK's wasm32 library tests for the browser key test
// (NOC-175 slice 2), and keeps the test module at a fixed path,
// target/sdk-keys/nocentraldb-lib-tests.wasm, which keys.e2e.test.mjs
// serves through the pinned wasm-bindgen-test-runner that the root task
// `wasm-runner` installs into target/tools. The browser's sync tests reach
// the local relay server at RELAY_URL, which the module is built with, so it
// also builds that server, target/sdk-keys/ncdb-relay, for the test to start.
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
/** The local relay server, as the browser's sync tests reach it. */
export const RELAY_URL = "ws://127.0.0.1:7711";
export const RELAY = join(ROOT, "target", "sdk-keys", "ncdb-relay");

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
      env: { ...process.env, NCDB_TEST_RELAY: RELAY_URL },
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
  buildRelay();
  return OUT;
};

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
  copyFileSync(join(ROOT, "target", "debug", "ncdb-relay"), RELAY);
};

/** The module built before, with SDK_KEYS_PREBUILT=1, or null. */
export const prebuilt = () => {
  if (process.env.SDK_KEYS_PREBUILT !== "1") return null;
  for (const built of [OUT, RELAY]) {
    if (!existsSync(built))
      throw new Error(`${built} is missing: run build.mjs first`);
  }
  return OUT;
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  console.log(buildTests());
}

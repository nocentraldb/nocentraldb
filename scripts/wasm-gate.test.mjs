// The wasm32 gate (NOC-10): ncdb-core builds for wasm32-unknown-unknown and
// runs its tests there, as part of `pnpm gates`, so no change can make the
// core depend on something the browser does not have (GENESIS.md,
// principle 7).
//
// The checks read the files that declare the gate: rust-toolchain.toml, the
// root moon.yml and crates/ncdb-core/moon.yml. The last test proves the
// detector fires: it runs the wasm build task's own command against a
// throwaway crate that uses a platform-only API, and asserts that the
// command fails for wasm32 while the same crate builds natively.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  globSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (file) => readFileSync(join(root, file), "utf8");
const WASM = "wasm32-unknown-unknown";
const RUNNER = "wasm-bindgen-test-runner";

const core = () => parse(read("crates/ncdb-core/moon.yml")).tasks ?? {};
const repo = () => parse(read("moon.yml")).tasks ?? {};

/** The `[toolchain]` keys of rust-toolchain.toml, values parsed as JSON. */
const toolchain = () => {
  const keys = {};
  for (const line of read("rust-toolchain.toml").split("\n")) {
    const match = line.match(/^(\w+)\s*=\s*(.+)$/);
    if (match) keys[match[1]] = JSON.parse(match[2]);
  }
  return keys;
};

const argv = (command) => command.trim().split(/\s+/);

test("rust-toolchain.toml pins the channel and lists the wasm32 target", () => {
  const { channel, targets } = toolchain();
  assert.match(channel, /^\d+\.\d+\.\d+$/, "the channel is not pinned");
  assert.ok(Array.isArray(targets), "rust-toolchain.toml lists no targets");
  assert.ok(targets.includes(WASM), `targets lack ${WASM}`);
});

test("ncdb-core's gates build the crate for wasm32", () => {
  const tasks = core();
  assert.ok(tasks["build-wasm"], "crates/ncdb-core/moon.yml has no build-wasm");
  assert.deepEqual(argv(tasks["build-wasm"].command), [
    "cargo",
    "build",
    "--locked",
    "-p",
    "$project",
    "--target",
    WASM,
  ]);
  assert.ok(tasks.gates?.deps?.includes("~:build-wasm"), "not in :gates");
});

test("ncdb-core's gates run the crate's tests under the pinned wasm runner", () => {
  const tasks = core();
  const testWasm = tasks["test-wasm"];
  assert.ok(testWasm, "crates/ncdb-core/moon.yml has no test-wasm");
  assert.deepEqual(argv(testWasm.command), [
    "cargo",
    "test",
    "--locked",
    "-p",
    "$project",
    "--target",
    WASM,
  ]);
  assert.ok(tasks.gates?.deps?.includes("~:test-wasm"), "not in :gates");

  // Cargo runs each wasm test binary through the runner the root project
  // installs, at one exact version, before the task starts.
  assert.deepEqual(testWasm.deps, ["repo:wasm-runner"]);
  const runner = testWasm.env?.CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER;
  assert.equal(runner, `$workspaceRoot/target/tools/bin/${RUNNER}`);

  const install = repo()["wasm-runner"];
  assert.ok(install, "the root moon.yml has no wasm-runner task");
  const args = argv(install.command);
  assert.deepEqual(args.slice(0, 3), ["cargo", "install", "wasm-bindgen-cli"]);
  assert.match(args[args.indexOf("--version") + 1] ?? "", /^=\d+\.\d+\.\d+$/);
  assert.ok(args.includes("--locked"), "the runner installs unlocked");
  assert.equal(args[args.indexOf("--root") + 1], "target/tools");
});

// wasm-bindgen-test runs only functions marked #[wasm_bindgen_test]; a
// plain #[test] is skipped on wasm32 and the runner still exits 0 ("no tests
// to run!"). So every Rust file of ncdb-core with a #[test] must route the
// attribute to wasm_bindgen_test when it compiles for wasm32.
const ROUTE =
  /#\[cfg\(target_arch = "wasm32"\)\]\s*use wasm_bindgen_test::wasm_bindgen_test as test;/;

test("every test of ncdb-core also runs on wasm32", () => {
  const files = globSync("crates/ncdb-core/{src,tests}/**/*.rs", { cwd: root });
  const withTests = files.filter((file) => read(file).includes("#[test]"));
  assert.ok(withTests.length > 0, "ncdb-core has no #[test] at all");
  for (const file of withTests) {
    assert.match(read(file), ROUTE, `${file}: #[test] skips wasm32`);
  }
});

test("the wasm build task fails on a crate that uses a platform-only API, which builds natively (needs cargo)", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "wasm-gate-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const files = {
    // The pinned toolchain, with the wasm32 target it lists.
    "rust-toolchain.toml": read("rust-toolchain.toml"),
    // A crate of its own, so the repository's workspace does not claim it.
    "Cargo.toml": `[package]
name = "platform-only"
version = "0.1.0"
edition = "2024"
publish = false

[workspace]
`,
    "Cargo.lock": `version = 4

[[package]]
name = "platform-only"
version = "0.1.0"
`,
    // std::os::unix exists on Unix hosts only. (std::fs itself compiles for
    // wasm32-unknown-unknown and fails only when called, which the test-wasm
    // task catches, not the build.)
    "src/lib.rs": `use std::os::unix::fs::PermissionsExt;

pub fn mode(path: &str) -> u32 {
    std::fs::metadata(path).map(|m| m.permissions().mode()).unwrap_or(0)
}
`,
  };
  for (const [file, text] of Object.entries(files)) {
    mkdirSync(join(dir, dirname(file)), { recursive: true });
    writeFileSync(join(dir, file), text);
  }

  const task = core()["build-wasm"];
  assert.ok(task, "crates/ncdb-core/moon.yml has no build-wasm");
  const command = argv(task.command).map((arg) =>
    arg === "$project" ? "platform-only" : arg,
  );
  const cargo = (args) =>
    spawnSync(args[0], args.slice(1), { cwd: dir, encoding: "utf8" });

  // Natively the crate builds, so the wasm failure below is the detector.
  const native = cargo(
    command.filter(
      (arg, i, all) => arg !== "--target" && all[i - 1] !== "--target",
    ),
  );
  assert.equal(native.error, undefined, "cargo must be on PATH for this test");
  assert.equal(native.status, 0, native.stderr);

  const wasm = cargo(command);
  assert.notEqual(wasm.status, 0, "the wasm build accepted std::os::unix");
  assert.match(wasm.stderr, /could not find `unix` in `os`/);
});

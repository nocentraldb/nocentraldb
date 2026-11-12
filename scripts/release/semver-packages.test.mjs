import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const script = join(
  dirname(fileURLToPath(import.meta.url)),
  "semver-packages.mjs",
);

/** A workspace tree in a temporary directory: path -> text. */
const tree = (t, files) => {
  const dir = mkdtempSync(join(tmpdir(), "release-semver-packages-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  for (const [file, text] of Object.entries(files)) {
    mkdirSync(join(dir, dirname(file)), { recursive: true });
    writeFileSync(join(dir, file), text);
  }
  return dir;
};

const crate = (name, extra = "") =>
  `[package]\nname = "${name}"\nversion.workspace = true\n${extra}`;

const run = (dir, ...args) =>
  spawnSync(process.execPath, [script, ...args], {
    cwd: dir,
    encoding: "utf8",
  });

/** The key=value lines of stdout, as an object. */
const outputs = (stdout) =>
  Object.fromEntries(
    stdout
      .trim()
      .split("\n")
      .filter(Boolean)
      .map((line) => line.split(/=(.*)/s).slice(0, 2)),
  );

test("S1 a publishable crate the base has is checked", (t) => {
  const base = tree(t, { "crates/core/Cargo.toml": crate("core") });
  const head = tree(t, { "crates/core/Cargo.toml": crate("core") });

  const result = run(head, "--baseline", base);

  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(outputs(result.stdout), { packages: "core", any: "true" });
});

test("S2 a publishable crate new in the head is skipped, and named", (t) => {
  const base = tree(t, { "crates/core/Cargo.toml": crate("core") });
  const head = tree(t, {
    "crates/core/Cargo.toml": crate("core"),
    "crates/plugin/Cargo.toml": crate("plugin"),
  });

  const result = run(head, "--baseline", base);

  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(outputs(result.stdout), { packages: "core", any: "true" });
  assert.match(result.stderr, /plugin.*not in the base/);
});

test("S3 a crate with publish = false is not checked", (t) => {
  const base = tree(t, {
    "crates/core/Cargo.toml": crate("core"),
    "crates/harness/Cargo.toml": crate("harness", "publish = false\n"),
  });
  const head = tree(t, {
    "crates/core/Cargo.toml": crate("core"),
    "crates/harness/Cargo.toml": crate("harness", "publish = false\n"),
  });

  const result = run(head, "--baseline", base);

  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(outputs(result.stdout), { packages: "core", any: "true" });
});

test("S4 a crate the base kept off crates.io is checked once publishable", (t) => {
  const base = tree(t, {
    "crates/plugin/Cargo.toml": crate("plugin", "publish = false\n"),
  });
  const head = tree(t, { "crates/plugin/Cargo.toml": crate("plugin") });

  const result = run(head, "--baseline", base);

  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(outputs(result.stdout), { packages: "plugin", any: "true" });
});

test("S5 no crate to check gives an empty list and any=false", (t) => {
  const base = tree(t, { "crates/old/Cargo.toml": crate("old") });
  const head = tree(t, { "crates/new/Cargo.toml": crate("new") });

  const result = run(head, "--baseline", base);

  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(outputs(result.stdout), { packages: "", any: "false" });
});

test("S6 several crates are listed comma-separated in name order", (t) => {
  const files = {
    "crates/b/Cargo.toml": crate("zeta"),
    "crates/a/Cargo.toml": crate("alpha"),
  };
  const base = tree(t, files);
  const head = tree(t, files);

  const result = run(head, "--baseline", base);

  assert.deepEqual(outputs(result.stdout), {
    packages: "alpha,zeta",
    any: "true",
  });
});

test("S7 a missing --baseline is a usage error", (t) => {
  const head = tree(t, { "crates/core/Cargo.toml": crate("core") });

  const result = run(head);

  assert.equal(result.status, 2);
});

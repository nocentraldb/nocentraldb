import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import {
  crateManifest,
  crateManifestFiles,
  lockVersions,
  packageJsonFiles,
  pinWorkspaceDependencies,
  readWorkspaceVersion,
  workspaceDependencies,
  writeWorkspaceVersion,
} from "./manifests.mjs";

const workspace = `[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.package]
version = "0.1.0"
edition = "2024"

[workspace.dependencies]
# inter-crate dependencies
ncdb-core = { path = "crates/ncdb-core", version = "=0.1.0" }
ncdb-sdk = { path = "crates/ncdb-sdk" }
serde = "1"
`;

test("readWorkspaceVersion reads the version of [workspace.package]", () => {
  assert.equal(readWorkspaceVersion(workspace), "0.1.0");
});

test("readWorkspaceVersion ignores a version in [package] of a single-crate file", () => {
  const single = `[package]\nname = "x"\nversion = "9.9.9"\n\n[workspace.package]\nversion = "0.2.0"\n`;
  assert.equal(readWorkspaceVersion(single), "0.2.0");
  assert.throws(
    () => readWorkspaceVersion(`[package]\nname = "x"\nversion = "9.9.9"\n`),
    /no \[workspace\.package\] table/,
  );
});

test("readWorkspaceVersion throws on a missing line and on two lines", () => {
  assert.throws(
    () => readWorkspaceVersion(`[workspace.package]\nedition = "2024"\n`),
    /\[workspace\.package\] has no version/,
  );
  assert.throws(
    () =>
      readWorkspaceVersion(
        `[workspace.package]\nversion = "0.1.0"\nversion = "0.2.0"\n`,
      ),
    /\[workspace\.package\] has more than one version/,
  );
});

test("the reader fails closed on syntax it does not understand", () => {
  assert.throws(
    () =>
      readWorkspaceVersion(`[workspace.package]\nversion = "0.1.0" # note\n`),
    /unsupported manifest syntax at line 2/,
  );
  assert.throws(
    () =>
      workspaceDependencies(
        `[workspace.dependencies]\nb = {\n  path = "crates/b" }\n`,
      ),
    /unsupported manifest syntax at line 2/,
  );
  assert.throws(
    () =>
      workspaceDependencies(`[workspace.dependencies.b]\npath = "crates/b"\n`),
    /unsupported manifest syntax at line 1/,
  );
  assert.throws(
    () =>
      crateManifest(`[package]\nname = "a"\n[dependencies]\nb.path = "../b"\n`),
    /unsupported manifest syntax at line 4/,
  );
});

test("multi-line values in tables the reader skips are stepped over", () => {
  const text = `[workspace]\nmembers = [\n  "crates/*",\n]\n\n[workspace.metadata]\nnote = """\n[workspace.package]\nversion = "9.9.9"\n"""\n\n[workspace.package]\nversion = "0.3.0"\n`;
  assert.equal(readWorkspaceVersion(text), "0.3.0");
});

test("writeWorkspaceVersion replaces exactly that line", () => {
  const out = writeWorkspaceVersion(workspace, "0.2.0-beta.1");
  const before = workspace.split("\n");
  const after = out.split("\n");
  assert.equal(after.length, before.length);
  for (let i = 0; i < before.length; i++) {
    if (before[i] === 'version = "0.1.0"')
      assert.equal(after[i], 'version = "0.2.0-beta.1"');
    else assert.equal(after[i], before[i]);
  }
});

test("workspaceDependencies lists the entries with a path, with or without a version", () => {
  assert.deepEqual(workspaceDependencies(workspace), [
    { name: "ncdb-core", version: "=0.1.0", path: "crates/ncdb-core" },
    { name: "ncdb-sdk", version: null, path: "crates/ncdb-sdk" },
  ]);
  assert.deepEqual(workspaceDependencies(`[workspace]\n`), []);
});

test("pinWorkspaceDependencies rewrites a pin and inserts a missing one", () => {
  const out = pinWorkspaceDependencies(workspace, "0.2.0-beta.1");
  assert.match(
    out,
    /^ncdb-core = \{ path = "crates\/ncdb-core", version = "=0\.2\.0-beta\.1" \}$/m,
  );
  assert.match(
    out,
    /^ncdb-sdk = \{ version = "=0\.2\.0-beta\.1", path = "crates\/ncdb-sdk" \}$/m,
  );
  assert.match(out, /^serde = "1"$/m);
  assert.match(out, /^version = "0\.1\.0"$/m);
  assert.equal(out.split("\n").length, workspace.split("\n").length);
});

test("crateManifest detects version.workspace = true", () => {
  assert.deepEqual(
    crateManifest(
      `[package]\nname = "ncdb-core"\nversion.workspace = true\nedition.workspace = true\n`,
    ),
    {
      name: "ncdb-core",
      inheritsVersion: true,
      ownVersion: null,
      pathDependencies: [],
    },
  );
});

test("crateManifest detects an own version and path dependencies in every dependency table", () => {
  const text = `[package]
name = "a"
version = "0.0.0"

[dependencies]
b = { path = "../b" }
c.workspace = true
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
d = { path = "../d", version = "0.1.0" }

[build-dependencies]
e = { workspace = true }

[target.'cfg(unix)'.dependencies]
f = { path = "../f" }
`;
  assert.deepEqual(crateManifest(text), {
    name: "a",
    inheritsVersion: false,
    ownVersion: "0.0.0",
    pathDependencies: ["b", "d", "f"],
  });
});

test("lockVersions reads every [[package]] block", () => {
  const lock = `# This file is automatically @generated by Cargo.
version = 4

[[package]]
name = "a"
version = "0.1.0"
dependencies = [
 "b",
]

[[package]]
name = "b"
version = "0.1.0-beta.1"
`;
  assert.deepEqual(
    lockVersions(lock),
    new Map([
      ["a", "0.1.0"],
      ["b", "0.1.0-beta.1"],
    ]),
  );
});

test("lockVersions prefers the workspace member when a name appears twice", () => {
  const lock = `[[package]]
name = "a"
version = "0.1.0"

[[package]]
name = "a"
version = "7.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
`;
  assert.equal(lockVersions(lock).get("a"), "0.1.0");
});

test("packageJsonFiles and crateManifestFiles list the workspace manifests", (t) => {
  const root = mkdtempSync(join(tmpdir(), "manifests-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const file of [
    "packages/sdk/package.json",
    "packages/b/package.json",
    "apps/web/package.json",
    "apps/web/node_modules/x/package.json",
    "package.json",
    "crates/a/Cargo.toml",
    "crates/b/Cargo.toml",
    "Cargo.toml",
  ]) {
    mkdirSync(join(root, dirname(file)), { recursive: true });
    writeFileSync(join(root, file), "{}");
  }
  assert.deepEqual(packageJsonFiles(root), [
    "apps/web/package.json",
    "packages/b/package.json",
    "packages/sdk/package.json",
  ]);
  assert.deepEqual(crateManifestFiles(root), [
    "crates/a/Cargo.toml",
    "crates/b/Cargo.toml",
  ]);
});

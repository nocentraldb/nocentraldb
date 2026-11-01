import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const script = join(
  dirname(fileURLToPath(import.meta.url)),
  "check-version.mjs",
);
const run = (cwd, ...args) =>
  spawnSync(process.execPath, [script, ...args], { cwd, encoding: "utf8" });

const CARGO = `[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.package]
version = "0.1.0"
edition = "2024"

[workspace.dependencies]
b = { path = "crates/b", version = "=0.1.0" }
`;
const CRATE_A = `[package]
name = "a"
version.workspace = true
edition.workspace = true

[dependencies]
b.workspace = true
`;
const CRATE_B = `[package]
name = "b"
version.workspace = true
edition.workspace = true
`;
const PACKAGE = `${JSON.stringify({ name: "@x/p", version: "0.1.0" }, null, 2)}\n`;
const LOCK = `version = 4

[[package]]
name = "a"
version = "0.1.0"
dependencies = [
 "b",
]

[[package]]
name = "b"
version = "0.1.0"
`;
const GREEN = {
  "Cargo.toml": CARGO,
  "crates/a/Cargo.toml": CRATE_A,
  "crates/b/Cargo.toml": CRATE_B,
  "packages/p/package.json": PACKAGE,
  "Cargo.lock": LOCK,
};

const fixture = (t, overrides = {}) => {
  const dir = mkdtempSync(join(tmpdir(), "check-version-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  for (const [file, text] of Object.entries({ ...GREEN, ...overrides })) {
    if (text === null) continue;
    mkdirSync(join(dir, dirname(file)), { recursive: true });
    writeFileSync(join(dir, file), text);
  }
  return dir;
};
const red = (r, pattern) => {
  assert.equal(r.status, 1, r.stderr);
  assert.match(r.stderr, pattern);
};

test("passes a tree whose manifests agree on the plain workspace version", (t) => {
  const r = run(fixture(t));
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stderr, "");
  assert.match(r.stdout, /check-version: 0\.1\.0/);
});

test("--help exits 0 and an unknown flag or a --tag without value exits 2", () => {
  const help = run(tmpdir(), "--help");
  assert.equal(help.status, 0);
  assert.match(help.stdout, /^usage: check-version/);
  assert.equal(run(tmpdir(), "--bogus").status, 2);
  assert.equal(run(tmpdir(), "--tag").status, 2);
});

test("R1: a stage suffix in the workspace version fails", (t) => {
  const r = run(
    fixture(t, {
      "Cargo.toml": CARGO.replace(
        'version = "0.1.0"',
        'version = "0.1.0-beta.1"',
      ),
    }),
  );
  red(
    r,
    /^check-version: Cargo\.toml: \[workspace\.package\] version "0\.1\.0-beta\.1" is not X\.Y\.Z: committed manifests never carry a stage suffix$/m,
  );
});

test("R1: build metadata in the workspace version fails", (t) => {
  const r = run(
    fixture(t, {
      "Cargo.toml": CARGO.replace('version = "0.1.0"', 'version = "0.1.0+abc"'),
    }),
  );
  red(
    r,
    /Cargo\.toml: \[workspace\.package\] version "0\.1\.0\+abc" is not X\.Y\.Z/,
  );
});

test("R1: a missing workspace version fails", (t) => {
  const r = run(
    fixture(t, {
      "Cargo.toml": CARGO.replace('version = "0.1.0"\n', ""),
    }),
  );
  red(
    r,
    /^check-version: Cargo\.toml: \[workspace\.package\] has no version$/m,
  );
});

test("R2: a crate with its own version fails", (t) => {
  const r = run(
    fixture(t, {
      "crates/b/Cargo.toml": CRATE_B.replace(
        "version.workspace = true",
        'version = "0.0.0"',
      ),
    }),
  );
  red(
    r,
    /^check-version: crates\/b\/Cargo\.toml: \[package\] declares its own version "0\.0\.0"; crates inherit the workspace version with version\.workspace = true$/m,
  );
});

test("R2: a crate without version.workspace = true fails", (t) => {
  const r = run(
    fixture(t, {
      "crates/b/Cargo.toml": CRATE_B.replace("version.workspace = true\n", ""),
    }),
  );
  red(
    r,
    /^check-version: crates\/b\/Cargo\.toml: \[package\] must inherit the workspace version with version\.workspace = true$/m,
  );
});

test("R3: a crate with a direct path dependency fails", (t) => {
  const r = run(
    fixture(t, {
      "crates/a/Cargo.toml": CRATE_A.replace(
        "b.workspace = true",
        'b = { path = "../b" }',
      ),
    }),
  );
  red(
    r,
    /^check-version: crates\/a\/Cargo\.toml: dependency b is declared with path =; declare dependencies between workspace crates in \[workspace\.dependencies\] of Cargo\.toml and use b\.workspace = true$/m,
  );
});

test("R4: a workspace dependency pinned to another version fails", (t) => {
  const r = run(
    fixture(t, {
      "Cargo.toml": CARGO.replace('version = "=0.1.0"', 'version = "0.1.0"'),
    }),
  );
  red(
    r,
    /^check-version: Cargo\.toml: \[workspace\.dependencies\] b must be pinned with version = "=0\.1\.0" \(found "0\.1\.0"\)$/m,
  );
});

test("R4: a workspace dependency without a version fails", (t) => {
  const r = run(
    fixture(t, {
      "Cargo.toml": CARGO.replace(', version = "=0.1.0"', ""),
    }),
  );
  red(
    r,
    /Cargo\.toml: \[workspace\.dependencies\] b must be pinned with version = "=0\.1\.0" \(found none\)/,
  );
});

test("R5: a package.json at another version fails", (t) => {
  const r = run(
    fixture(t, {
      "packages/p/package.json": PACKAGE.replace("0.1.0", "0.0.0"),
      "apps/w/package.json": `{ "name": "w" }\n`,
    }),
  );
  red(
    r,
    /^check-version: packages\/p\/package\.json: version "0\.0\.0" is not the workspace version "0\.1\.0"$/m,
  );
  assert.match(
    r.stderr,
    /^check-version: apps\/w\/package\.json: version none is not the workspace version "0\.1\.0"$/m,
  );
});

test("R6: Cargo.lock listing a crate at another version fails", (t) => {
  const r = run(
    fixture(t, {
      "Cargo.lock": LOCK.replace(
        'name = "b"\nversion = "0.1.0"',
        'name = "b"\nversion = "0.0.0"',
      ),
    }),
  );
  red(
    r,
    /^check-version: Cargo\.lock: b is at 0\.0\.0, not at the workspace version 0\.1\.0 \(run cargo update --workspace --offline\)$/m,
  );
});

test("R6: Cargo.lock without a workspace crate, or no Cargo.lock at all, fails", (t) => {
  const missing = run(
    fixture(t, {
      "Cargo.lock": LOCK.replace(/\n\[\[package\]\]\nname = "b"[\s\S]*$/, "\n"),
    }),
  );
  red(
    missing,
    /^check-version: Cargo\.lock: does not list the workspace crate b/m,
  );
  const none = run(fixture(t, { "Cargo.lock": null }));
  red(none, /^check-version: Cargo\.lock: missing/m);
});

test("R7: --tag passes a beta or release tag of the workspace version", (t) => {
  const dir = fixture(t);
  assert.equal(run(dir, "--tag", "v0.1.0-beta.2").status, 0);
  assert.equal(run(dir, "--tag", "v0.1.0").status, 0);
});

test("R7: --tag fails a tag of another version", (t) => {
  red(
    run(fixture(t), "--tag", "v0.2.0-beta.1"),
    /^check-version: tag v0\.2\.0-beta\.1: its version 0\.2\.0 is not the workspace version 0\.1\.0$/m,
  );
});

test("R7: --tag fails an unknown tag shape", (t) => {
  red(
    run(fixture(t), "--tag", "v0.1.0-rc1"),
    /^check-version: tag v0\.1\.0-rc1: unknown tag shape \(expected vX\.Y\.Z or vX\.Y\.Z-beta\.N\)$/m,
  );
});

test("R7: --tag fails an alpha tag", (t) => {
  red(
    run(fixture(t), "--tag", "v0.1.0-alpha.1"),
    /^check-version: tag v0\.1\.0-alpha\.1: alpha tags are not release tags/m,
  );
});

test("a manifest the reader cannot parse fails with its path", (t) => {
  const r = run(
    fixture(t, {
      "crates/a/Cargo.toml": `${CRATE_A}[dependencies.c]\npath = "../c"\n`,
    }),
  );
  red(
    r,
    /^check-version: crates\/a\/Cargo\.toml: unsupported manifest syntax at line 8$/m,
  );
});

test("a missing Cargo.toml fails", (t) => {
  red(
    run(fixture(t, { "Cargo.toml": null })),
    /^check-version: Cargo\.toml: missing/m,
  );
});

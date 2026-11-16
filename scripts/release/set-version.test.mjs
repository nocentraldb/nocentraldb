import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
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
import { lockVersions } from "./lib/manifests.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const script = join(here, "set-version.mjs");
const checkVersion = join(here, "check-version.mjs");
const toolchain = readFileSync(
  join(here, "..", "..", "rust-toolchain.toml"),
  "utf8",
);
const run = (cwd, ...args) =>
  spawnSync(process.execPath, [script, ...args], { cwd, encoding: "utf8" });

const fixture = (t, files) => {
  const dir = mkdtempSync(join(tmpdir(), "set-version-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  for (const [file, text] of Object.entries(files)) {
    mkdirSync(join(dir, dirname(file)), { recursive: true });
    writeFileSync(join(dir, file), text);
  }
  return dir;
};
const read = (dir, file) => readFileSync(join(dir, file), "utf8");

const CARGO = `[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
# Dependencies between workspace crates.
b = { path = "crates/b", version = "=0.1.0" }
c = { path = "crates/c" }
serde = "1"
`;
const crate = (name, deps = "") =>
  `[package]\nname = "${name}"\nversion.workspace = true\nedition.workspace = true\n${deps}`;
const PACKAGE = `${JSON.stringify(
  {
    name: "@x/p",
    version: "0.1.0",
    private: true,
    type: "module",
    main: "./src/index.ts",
  },
  null,
  2,
)}\n`;

test("--no-lock rewrites the workspace version, the pins and every package.json, and nothing else", (t) => {
  const files = {
    "Cargo.toml": CARGO,
    "crates/a/Cargo.toml": crate("a", "\n[dependencies]\nb.workspace = true\n"),
    "crates/b/Cargo.toml": crate("b"),
    "packages/p/package.json": PACKAGE,
  };
  const dir = fixture(t, files);
  const r = run(dir, "0.2.0-beta.1", "--no-lock");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout, "set-version: 0.2.0-beta.1 written to 2 manifests\n");

  const expected = CARGO.replace(
    'version = "0.1.0"',
    'version = "0.2.0-beta.1"',
  )
    .replace('version = "=0.1.0"', 'version = "=0.2.0-beta.1"')
    .replace(
      'c = { path = "crates/c" }',
      'c = { version = "=0.2.0-beta.1", path = "crates/c" }',
    );
  const after = read(dir, "Cargo.toml").split("\n");
  const want = expected.split("\n");
  assert.equal(after.length, want.length);
  for (let i = 0; i < want.length; i++) assert.equal(after[i], want[i]);

  const pkgAfter = read(dir, "packages/p/package.json").split("\n");
  const pkgWant = PACKAGE.replace('"0.1.0"', '"0.2.0-beta.1"').split("\n");
  assert.equal(pkgAfter.length, pkgWant.length);
  for (let i = 0; i < pkgWant.length; i++)
    assert.equal(pkgAfter[i], pkgWant[i]);

  assert.equal(read(dir, "crates/a/Cargo.toml"), files["crates/a/Cargo.toml"]);
  assert.equal(read(dir, "crates/b/Cargo.toml"), files["crates/b/Cargo.toml"]);
});

test("a package.json keeps every byte but its version value, and a second run changes nothing", (t) => {
  const odd = `{
    "name": "@x/odd",
    "devEngines": { "runtime": { "name": "node", "version": ">=24" } },
    "version": "0.1.0",
    "scripts": {"build": "tsc"}
}`;
  const dir = fixture(t, {
    "Cargo.toml": CARGO,
    "packages/odd/package.json": odd,
    "packages/p/package.json": PACKAGE,
  });
  assert.equal(run(dir, "0.2.0", "--no-lock").status, 0);
  const once = read(dir, "packages/odd/package.json");
  assert.equal(once, odd.replace('"version": "0.1.0"', '"version": "0.2.0"'));
  const files = [
    "Cargo.toml",
    "packages/odd/package.json",
    "packages/p/package.json",
  ];
  const before = files.map((file) => read(dir, file));
  const r = run(dir, "0.2.0", "--no-lock");
  assert.equal(r.status, 0, r.stderr);
  assert.deepEqual(
    files.map((file) => read(dir, file)),
    before,
  );
});

test("a Cargo.lock it cannot read fails with its path and writes nothing", (t) => {
  const broken = '[[package]]\nname = "a"\n';
  const dir = fixture(t, {
    "Cargo.toml": CARGO,
    "Cargo.lock": broken,
    "packages/p/package.json": PACKAGE,
  });
  const r = run(dir, "0.2.0");
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /^set-version: Cargo\.lock: the \[\[package\]\] block at line 1 has no name or no version$/m,
  );
  assert.equal(read(dir, "Cargo.toml"), CARGO);
  assert.equal(read(dir, "Cargo.lock"), broken);
  assert.equal(read(dir, "packages/p/package.json"), PACKAGE);
  assert.equal(r.stdout, "");
});

test("an invalid version fails and writes nothing", (t) => {
  const dir = fixture(t, {
    "Cargo.toml": CARGO,
    "packages/p/package.json": PACKAGE,
  });
  for (const bad of ["v0.2.0", "0.2.0-rc.1", "0.2"]) {
    const r = run(dir, bad, "--no-lock");
    assert.equal(r.status, 1);
    assert.match(r.stderr, new RegExp(`^set-version: not a version: "${bad}"`));
  }
  assert.equal(read(dir, "Cargo.toml"), CARGO);
  assert.equal(read(dir, "packages/p/package.json"), PACKAGE);
});

test("an unreadable Cargo.toml fails with its path and writes nothing", (t) => {
  const broken = CARGO.replace('version = "0.1.0"\n', "");
  const dir = fixture(t, {
    "Cargo.toml": broken,
    "packages/p/package.json": PACKAGE,
  });
  const r = run(dir, "0.2.0", "--no-lock");
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /^set-version: Cargo\.toml: \[workspace\.package\] has no version$/m,
  );
  assert.equal(read(dir, "Cargo.toml"), broken);
  assert.equal(read(dir, "packages/p/package.json"), PACKAGE);
});

test("--help exits 0; no version or an unknown flag exits 2", () => {
  const help = run(tmpdir(), "--help");
  assert.equal(help.status, 0);
  assert.match(help.stdout, /^usage: set-version/);
  assert.equal(run(tmpdir()).status, 2);
  assert.equal(run(tmpdir(), "0.1.0", "--bogus").status, 2);
  assert.equal(run(tmpdir(), "0.1.0", "0.2.0").status, 2);
});

test("without --no-lock a prerelease version resolves between two crates, Cargo.lock follows it and cargo accepts it (needs cargo)", (t) => {
  const dir = fixture(t, {
    // The repository's pinned toolchain, which the gate installs.
    "rust-toolchain.toml": toolchain,
    "Cargo.toml": `[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.package]
version = "0.1.0"
edition = "2024"

[workspace.dependencies]
b = { path = "crates/b", version = "=0.1.0" }
`,
    "crates/a/Cargo.toml": crate("a", "\n[dependencies]\nb.workspace = true\n"),
    "crates/a/src/lib.rs": "pub use b::B;\n",
    "crates/b/Cargo.toml": crate("b"),
    "crates/b/src/lib.rs": "pub struct B;\n",
  });
  const lock = spawnSync("cargo", ["generate-lockfile", "--offline"], {
    cwd: dir,
    encoding: "utf8",
  });
  assert.equal(
    lock.status,
    0,
    `cargo must be on PATH for this test: ${lock.error ?? lock.stderr}`,
  );
  assert.deepEqual(
    lockVersions(read(dir, "Cargo.lock")),
    new Map([
      ["a", "0.1.0"],
      ["b", "0.1.0"],
    ]),
  );

  const r = run(dir, "0.1.0-beta.1");
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /set-version: 0\.1\.0-beta\.1 written to 1 manifests/);
  assert.deepEqual(
    lockVersions(read(dir, "Cargo.lock")),
    new Map([
      ["a", "0.1.0-beta.1"],
      ["b", "0.1.0-beta.1"],
    ]),
  );
  assert.match(
    read(dir, "Cargo.toml"),
    /^b = \{ path = "crates\/b", version = "=0\.1\.0-beta\.1" \}$/m,
  );
  // cargo takes the rewritten lock as it is: --locked fails on a lock it
  // would change.
  const locked = spawnSync(
    "cargo",
    ["metadata", "--locked", "--offline", "--format-version", "1"],
    { cwd: dir, encoding: "utf8" },
  );
  assert.equal(locked.status, 0, locked.stderr);

  // The checkout would pass check-version but for R1, which keeps the stage
  // suffix out of committed manifests.
  const check = spawnSync(process.execPath, [checkVersion], {
    cwd: dir,
    encoding: "utf8",
  });
  assert.equal(check.status, 1);
  assert.deepEqual(check.stderr.trim().split("\n"), [
    'check-version: Cargo.toml: [workspace.package] version "0.1.0-beta.1" is not X.Y.Z: committed manifests never carry a stage suffix',
  ]);
});

// A Cargo.lock with a third-party crate from crates.io beside the two
// workspace members, as the repository's own lock has (NOC-281: the alpha-npm
// job failed on futures-core).
const REGISTRY = `source = "registry+https://github.com/rust-lang/crates.io-index"`;
const LOCK = `# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

[[package]]
name = "a"
version = "0.1.0"
dependencies = [
 "b",
 "itoa",
]

[[package]]
name = "b"
version = "0.1.0"

[[package]]
name = "itoa"
version = "1.0.15"
${REGISTRY}
checksum = "4a5f13b858c8d314ee3e8f639011f7ccefe71f97f96e50151fb991f267928e2c"
`;

test("Cargo.lock follows the version without a cargo registry cache, as in a fresh CI job", (t) => {
  const dir = fixture(t, {
    "rust-toolchain.toml": toolchain,
    "Cargo.toml": CARGO.replace("serde", "itoa"),
    "Cargo.lock": LOCK,
    "crates/a/Cargo.toml": crate(
      "a",
      "\n[dependencies]\nb.workspace = true\nitoa.workspace = true\n",
    ),
    "crates/a/src/lib.rs": "",
    "crates/b/Cargo.toml": crate("b"),
    "crates/b/src/lib.rs": "",
    "packages/p/package.json": PACKAGE,
  });
  // An empty cargo home: no registry index and no downloaded crate.
  const cargoHome = join(dir, "empty-cargo-home");
  mkdirSync(cargoHome);
  const r = spawnSync(process.execPath, [script, "0.1.0-alpha.69"], {
    cwd: dir,
    encoding: "utf8",
    env: { ...process.env, CARGO_HOME: cargoHome },
  });
  assert.equal(r.status, 0, r.stderr);
  assert.equal(
    read(dir, "Cargo.lock"),
    LOCK.replaceAll(
      'name = "a"\nversion = "0.1.0"',
      'name = "a"\nversion = "0.1.0-alpha.69"',
    ).replace(
      'name = "b"\nversion = "0.1.0"',
      'name = "b"\nversion = "0.1.0-alpha.69"',
    ),
  );
});

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

const lib = pathToFileURL(
  join(dirname(fileURLToPath(import.meta.url)), "cli.mjs"),
).href;

/** Runs a tiny script that parses `args` with the given spec and prints the result. */
const parse = (spec, ...args) =>
  spawnSync(
    process.execPath,
    [
      "--input-type=module",
      "-e",
      `import { cli } from ${JSON.stringify(lib)};
       const { parseArgs } = cli("demo", "usage: demo");
       console.log(JSON.stringify(parseArgs(process.argv.slice(1), ${JSON.stringify(spec)})));`,
      "--",
      ...args,
    ],
    { encoding: "utf8" },
  );

const SPEC = {
  flags: ["--json", "--dry-run"],
  values: ["--tag", "--head"],
  defaults: { head: "HEAD", json: false },
};

test("parseArgs reads flags as true and values as strings, camel-cased, over the defaults", () => {
  const r = parse(SPEC, "--dry-run", "--tag", "v1.0.0");
  assert.equal(r.status, 0, r.stderr);
  assert.deepEqual(JSON.parse(r.stdout), {
    head: "HEAD",
    json: false,
    dryRun: true,
    tag: "v1.0.0",
  });
});

test("parseArgs: --help prints the usage on stdout and exits 0", () => {
  const r = parse(SPEC, "--tag", "x", "--help");
  assert.equal(r.status, 0);
  assert.equal(r.stdout, "usage: demo\n");
});

test("parseArgs: an unknown argument, a positional or a value flag without its value is a usage error", () => {
  for (const args of [["--nope"], ["stray"], ["--tag"], ["--tag", ""]]) {
    const r = parse(SPEC, ...args);
    assert.equal(r.status, 2, args.join(" "));
    assert.equal(r.stdout, "", args.join(" "));
    assert.equal(r.stderr, "usage: demo\n", args.join(" "));
  }
});

test("fail prints <name>: <message> on stderr, sets exit code 1 and keeps running", () => {
  const r = spawnSync(
    process.execPath,
    [
      "--input-type=module",
      "-e",
      `import { cli } from ${JSON.stringify(lib)};
       const { fail } = cli("demo", "usage: demo");
       fail("first"); fail("second"); console.log("still here");`,
    ],
    { encoding: "utf8" },
  );
  assert.equal(r.status, 1);
  assert.equal(r.stderr, "demo: first\ndemo: second\n");
  assert.equal(r.stdout, "still here\n");
});

test("usageError prints the usage on stderr and exits 2", () => {
  const r = spawnSync(
    process.execPath,
    [
      "--input-type=module",
      "-e",
      `import { cli } from ${JSON.stringify(lib)};
       cli("demo", "usage: demo").usageError(); console.log("not reached");`,
    ],
    { encoding: "utf8" },
  );
  assert.equal(r.status, 2);
  assert.equal(r.stderr, "usage: demo\n");
  assert.equal(r.stdout, "");
});

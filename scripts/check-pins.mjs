// Fails when a tool or npm dependency is not pinned to one exact version:
// - .prototools must pin node and pnpm;
// - .nvmrc must equal the node pin in .prototools;
// - packageManager in the root package.json must equal pnpm@<pnpm pin>;
// - every package.json dependency must be an exact version (x.y.z with an
//   optional prerelease/build suffix) or a workspace: reference to a project
//   in this repository. Ranges, tags, URLs, git, file: and npm: aliases fail.
// Node built-ins only, so it runs before `pnpm install`. Paths are relative
// to the working directory, the repository root (scripts/check-pins.test.mjs
// runs it in fixture trees).
import { globSync, readFileSync } from "node:fs";

const fail = (message) => {
  console.error(`check-pins: ${message}`);
  process.exitCode = 1;
};

const read = (file) => {
  try {
    return readFileSync(file, "utf8");
  } catch (error) {
    fail(`cannot read ${file}: ${error.code ?? error.message}`);
    return null;
  }
};

const readJson = (file) => {
  const text = read(file);
  if (text === null) return null;
  try {
    return JSON.parse(text);
  } catch (error) {
    fail(`${file} is not valid JSON: ${error.message}`);
    return null;
  }
};

const EXACT = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/;
const WORKSPACE = /^workspace:/;

const proto = Object.fromEntries(
  (read(".prototools") ?? "")
    .split("\n")
    .map((line) => line.match(/^(\w+)\s*=\s*"([^"]+)"/))
    .filter(Boolean)
    .map((match) => [match[1], match[2]]),
);
for (const key of ["node", "pnpm"]) {
  if (!proto[key]) fail(`.prototools has no ${key} pin`);
}

const nvmrc = read(".nvmrc")?.trim();
if (nvmrc !== undefined && proto.node && nvmrc !== proto.node) {
  fail(`.nvmrc ${nvmrc} != .prototools node ${proto.node}`);
}

const root = readJson("package.json");
if (root && proto.pnpm && root.packageManager !== `pnpm@${proto.pnpm}`) {
  fail(
    `package.json packageManager ${root.packageManager} != pnpm@${proto.pnpm} from .prototools`,
  );
}

const fields = [
  "dependencies",
  "devDependencies",
  "peerDependencies",
  "optionalDependencies",
];
for (const file of [
  "package.json",
  ...globSync("{apps,packages}/*/package.json").sort(),
]) {
  const pkg = file === "package.json" ? root : readJson(file);
  if (!pkg) continue;
  for (const field of fields) {
    for (const [dep, spec] of Object.entries(pkg[field] ?? {})) {
      if (!EXACT.test(spec) && !WORKSPACE.test(spec)) {
        fail(`${file} ${field}.${dep} = "${spec}" is not exact`);
      }
    }
  }
}

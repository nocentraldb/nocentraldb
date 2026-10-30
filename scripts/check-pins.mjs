// Fails when a tool or npm dependency is not pinned to one exact version:
// - .nvmrc must equal the node pin in .prototools;
// - packageManager in the root package.json must equal pnpm@<pnpm pin>;
// - no package.json may declare a dependency range.
// Node built-ins only, so it runs before `pnpm install`.
import { globSync, readFileSync } from "node:fs";

const fail = (message) => {
  console.error(`check-pins: ${message}`);
  process.exitCode = 1;
};

const proto = Object.fromEntries(
  readFileSync(".prototools", "utf8")
    .split("\n")
    .map((line) => line.match(/^(\w+)\s*=\s*"([^"]+)"/))
    .filter(Boolean)
    .map((match) => [match[1], match[2]]),
);

const nvmrc = readFileSync(".nvmrc", "utf8").trim();
if (nvmrc !== proto.node) {
  fail(`.nvmrc ${nvmrc} != .prototools node ${proto.node}`);
}

const root = JSON.parse(readFileSync("package.json", "utf8"));
if (root.packageManager !== `pnpm@${proto.pnpm}`) {
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
  ...globSync("{apps,packages}/*/package.json"),
]) {
  const pkg = JSON.parse(readFileSync(file, "utf8"));
  for (const field of fields) {
    for (const [dep, spec] of Object.entries(pkg[field] ?? {})) {
      if (/^[\^~><*]|\s|\|\|/.test(spec)) {
        fail(`${file} ${field}.${dep} = "${spec}" is not exact`);
      }
    }
  }
}

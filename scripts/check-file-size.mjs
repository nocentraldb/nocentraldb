// Fails when a tracked code or test file is longer than the limit that
// factory/CONFIG.md sets in its row "Largest code or test file" (NOC-43).
// Code is every file git tracks with one of the extensions below, outside
// node_modules/ and target/. Runs from the repository root
// (scripts/check-file-size.test.mjs runs it in fixture repositories). Node
// built-ins and git only.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const CONFIG = "factory/CONFIG.md";
const ROW = /^\|\s*Largest code or test file\s*\|\s*(\d+) lines\b/m;
const CODE = /\.(rs|ts|tsx|js|mjs|cjs|sh|py)$/;
// Not written by hand: installed packages and build output, at any depth.
// Lock files and docs carry no code extension, so CODE leaves them out.
const GENERATED = /(^|\/)(node_modules|target)\//;
const isChecked = (path) => CODE.test(path) && !GENERATED.test(path);

/** Lines as wc counts them, plus a last line that has no newline. */
const countLines = (text) => {
  const breaks = text.split("\n").length - 1;
  return text === "" || text.endsWith("\n") ? breaks : breaks + 1;
};

const fail = (message) => {
  console.error(`file-size: ${message}`);
  process.exitCode = 1;
};

const limitRow = readFileSync(CONFIG, "utf8").match(ROW);
if (!limitRow) {
  fail(
    `${CONFIG} has no row "| Largest code or test file | <number> lines ... |"`,
  );
  process.exit();
}
const LIMIT = Number(limitRow[1]);

const tracked = execFileSync("git", ["ls-files", "-z"], { encoding: "utf8" })
  .split("\0")
  .filter(Boolean);

for (const file of tracked.filter(isChecked)) {
  const count = countLines(readFileSync(file, "utf8"));
  if (count > LIMIT) {
    fail(
      `${file} has ${count} lines; the limit is ${LIMIT} (${CONFIG}). ` +
        "Split it by concern (factory/FACTORY.md, " +
        '"Small files, one concern each").',
    );
  }
}

// Runs `ncdb` both ways on the same project (NOC-415): the native binary
// that `cargo install ncdb` installs, and the bin @nocentraldb/sdk ships,
// which runs crates/ncdb compiled to wasm. The tests compare what both say
// and write.
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const PACKAGE = join(dirname(fileURLToPath(import.meta.url)), "..");
const ROOT = join(PACKAGE, "..", "..");
const SDK = join(PACKAGE, "src", "index.ts");

/** What a run printed and wrote. */
export type Run = {
  code: number | null;
  stdout: string;
  stderr: string;
  /** The lock after the run, or null when there is none. */
  lock: string | null;
};

const exec = (command: string, args: string[], cwd = ROOT) => {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed:\n${result.stdout}${result.stderr}`,
    );
  }
};

/**
 * Builds both: the native binary with cargo, and the bin's wasm core with
 * the SDK's own build step, next to a script that starts the bin on it as
 * dist/ncdb.js does. Returns the two commands.
 */
export const buildBoth = (): { native: string; npm: string } => {
  exec("cargo", ["build", "--locked", "-p", "ncdb"]);
  const out = mkdtempSync(join(tmpdir(), "ncdb-npm-"));
  exec("node", [join(PACKAGE, "build-ncdb.mjs"), join(out, "ncdb")]);
  const cli = pathToFileURL(join(PACKAGE, "src", "ncdbCli.ts")).href;
  writeFileSync(
    join(out, "ncdb.mjs"),
    `import { start } from ${JSON.stringify(cli)};\nawait start(new URL("./ncdb/", import.meta.url));\n`,
  );
  return {
    native: join(ROOT, "target", "debug", "ncdb"),
    npm: join(out, "ncdb.mjs"),
  };
};

/** A schema module that imports this repository's SDK. */
export const schema = (body: string): string =>
  `import { nc } from ${JSON.stringify(SDK)};\n${body}`;

/** A project: its files, by path relative to the project. */
export type Files = Record<string, string>;

const project = (files: Files): string => {
  const dir = mkdtempSync(join(tmpdir(), "ncdb-project-"));
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(dirname(join(dir, path)), { recursive: true });
    writeFileSync(join(dir, path), text);
  }
  return dir;
};

/** Runs `command` in a new project made of `files`. */
export const runIn = (
  command: { native: string } | { npm: string },
  files: Files,
  args: string[],
  lock = "model.lock.json",
): Run => {
  const dir = project(files);
  const [program, first] =
    "native" in command
      ? [command.native, []]
      : [process.execPath, [command.npm]];
  const result = spawnSync(program, [...first, ...args], {
    cwd: dir,
    encoding: "utf8",
  });
  const path = join(dir, lock);
  return {
    code: result.status,
    stdout: result.stdout,
    stderr: result.stderr,
    lock: existsSync(path) ? readFileSync(path, "utf8") : null,
  };
};

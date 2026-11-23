/// <reference types="node" />
// The `ncdb` bin of @nocentraldb/sdk (NOC-415; Carsten, 2026-11-23,
// NOC-443, A): `npx ncdb model build` and `npx ncdb model check` without a
// Rust toolchain. It runs crates/ncdb compiled to wasm (crate ncdb-wasm,
// built by build-ncdb.mjs into dist/ncdb/) on the host below, so the
// arguments, the schema run, the lock and every message are the native
// `ncdb`'s. The host runs the schema with the Node.js that runs this bin,
// where the native `ncdb` takes `node` from PATH.
//
// Not part of the SDK's API: index.ts does not export it.
import { spawnSync } from "node:child_process";
import {
  readFileSync,
  realpathSync,
  renameSync,
  statSync,
  writeFileSync,
} from "node:fs";

/** What crates/ncdb asks of the machine (crates/ncdb/src/host.rs). */
const host = {
  node(args: string[]) {
    const result = spawnSync(process.execPath, args, {
      encoding: "utf8",
      maxBuffer: Number.POSITIVE_INFINITY,
    });
    if (result.error) throw result.error;
    return {
      success: result.status === 0,
      stdout: result.stdout,
      stderr: result.stderr,
    };
  },
  isFile(path: string): boolean {
    try {
      return statSync(path).isFile();
    } catch {
      return false;
    }
  },
  canonicalize: (path: string): string => realpathSync(path),
  read: (path: string): string => readFileSync(path, "utf8"),
  write: (path: string, text: string): void => writeFileSync(path, text),
  rename: (from: string, to: string): void => renameSync(from, to),
};

/** The module wasm-bindgen generates from ncdb-wasm. */
type Core = {
  initSync(options: { module: BufferSource }): unknown;
  run(
    args: string[],
    host: object,
  ): { code: number; stdout: string; stderr: string };
};

/**
 * Runs `ncdb` with this process's arguments on the core in `dir`, prints
 * what it says and sets the exit code.
 */
export const start = async (dir: URL): Promise<void> => {
  const core = (await import(new URL("ncdb_wasm.js", dir).href)) as Core;
  core.initSync({ module: readFileSync(new URL("ncdb_wasm_bg.wasm", dir)) });
  const outcome = core.run(process.argv.slice(2), host);
  process.stdout.write(outcome.stdout);
  process.stderr.write(outcome.stderr);
  process.exitCode = outcome.code;
};

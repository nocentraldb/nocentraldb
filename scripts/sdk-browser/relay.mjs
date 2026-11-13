// Starts the local relay server the TypeScript SDK's sync tests reach
// (NOC-176 slice 4), on a free port, with its database in a fresh
// directory. build.mjs builds it.
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { RELAY } from "./build.mjs";

/** Starts the relay; resolves to { url, stop } once it listens. `allow` lists `--allow-*` arguments. */
export const startRelay = async (allow = []) => {
  const dir = mkdtempSync(join(tmpdir(), "sdk-browser-relay-"));
  const relay = spawn(
    RELAY,
    [
      "--database",
      join(dir, "relay.sqlite"),
      "--listen",
      "127.0.0.1:0",
      ...allow,
    ],
    { stdio: ["ignore", "pipe", "inherit"] },
  );
  const url = await new Promise((resolve, reject) => {
    let out = "";
    relay.stdout.on("data", (chunk) => {
      out += chunk;
      const found = out.match(/ws:\/\/[^\s]+/);
      if (found) resolve(found[0]);
    });
    relay.on("exit", (code) =>
      reject(new Error(`ncdb-relay ended (${code}): ${out}`)),
    );
  });
  return {
    url,
    stop: () => {
      relay.kill();
      rmSync(dir, { recursive: true, force: true });
    },
  };
};

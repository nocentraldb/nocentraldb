// The Rust device of the end-to-end test with a browser (NOC-178): the
// example crates/nocentraldb/examples/browser_peer.rs, which it builds into
// target/sdk-browser/browser_peer and drives one command per line; and a
// proxy in front of the relay that the test can cut, as the Rust SDK's own
// sync tests cut theirs.
import { spawn, spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { createConnection, createServer } from "node:net";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { ROOT } from "./build.mjs";

/** The Rust device the test runs. */
export const PEER = join(ROOT, "target", "sdk-browser", "browser_peer");

/** Builds the Rust device into PEER. */
export const buildPeer = () => {
  const result = spawnSync(
    "cargo",
    ["build", "--locked", "-p", "nocentraldb", "--example", "browser_peer"],
    { cwd: ROOT, stdio: ["ignore", "inherit", "inherit"] },
  );
  if (result.status !== 0) {
    throw new Error(`cargo build of browser_peer failed (${result.status})`);
  }
  mkdirSync(dirname(PEER), { recursive: true });
  copyFileSync(join(ROOT, "target", "debug", "examples", "browser_peer"), PEER);
};

/** The Rust device built before; fails at once when it is missing. */
export const prebuiltPeer = () => {
  if (!existsSync(PEER)) {
    throw new Error(`${PEER} is missing: run peer.mjs first`);
  }
};

/**
 * Starts the Rust device on `lock`, syncing with `url`. Resolves to its
 * invitation and `ask(command)`, which resolves to the text after `ok` and
 * rejects on `err`.
 */
export const startPeer = async (lock, url) => {
  const child = spawn(PEER, [lock, url], {
    stdio: ["pipe", "pipe", "inherit"],
  });
  const lines = createInterface({ input: child.stdout });
  const waiting = [];
  const queued = [];
  lines.on("line", (line) => {
    const next = waiting.shift();
    if (next) next(line);
    else queued.push(line);
  });
  const ended = new Promise((_, reject) =>
    child.on("exit", (code) =>
      reject(new Error(`browser_peer ended (${code})`)),
    ),
  );
  ended.catch(() => {});
  const line = () =>
    Promise.race([
      new Promise((resolve) => {
        if (queued.length > 0) resolve(queued.shift());
        else waiting.push(resolve);
      }),
      ended,
    ]);
  const first = await line();
  const invitation = first.match(/^invitation (\S+)$/)?.[1];
  if (!invitation) throw new Error(`browser_peer started with: ${first}`);
  const ask = async (command) => {
    child.stdin.write(`${command}\n`);
    const answer = await line();
    if (answer === "ok") return "";
    if (answer.startsWith("ok ")) return answer.slice(3);
    throw new Error(`browser_peer: ${command.split(" ")[0]}: ${answer}`);
  };
  return {
    invitation,
    ask,
    stop: () => {
      child.stdin.end("quit\n");
      child.kill();
    },
  };
};

/**
 * A TCP proxy on a free port. It holds new connections back until `lead`
 * names the relay's address; `cut` drops every connection and refuses new
 * ones until `restore`.
 */
export const startProxy = async () => {
  let target;
  let up = true;
  const links = new Set();
  const held = [];
  const join_ = (inbound) => {
    const outbound = createConnection(target);
    links.add(inbound);
    links.add(outbound);
    const drop = () => {
      inbound.destroy();
      outbound.destroy();
      links.delete(inbound);
      links.delete(outbound);
    };
    inbound.on("error", drop).on("close", drop);
    outbound.on("error", drop).on("close", drop);
    inbound.pipe(outbound);
    outbound.pipe(inbound);
  };
  const server = createServer((inbound) => {
    if (!up) return inbound.destroy();
    if (!target) {
      held.push(inbound);
      inbound.on("error", () => {});
      return;
    }
    join_(inbound);
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  return {
    url: `ws://127.0.0.1:${port}`,
    /** Leads connections to the relay at the `ws://host:port` URL `url`. */
    lead: (url) => {
      const { hostname, port } = new URL(url);
      target = { host: hostname, port: Number(port) };
      for (const inbound of held.splice(0)) {
        if (!inbound.destroyed) join_(inbound);
      }
    },
    cut: () => {
      up = false;
      for (const socket of links) socket.destroy();
      links.clear();
    },
    restore: () => {
      up = true;
    },
    close: () =>
      new Promise((resolve) => {
        for (const socket of links) socket.destroy();
        server.close(resolve);
      }),
  };
};

if (process.argv[1] === import.meta.filename) buildPeer();

// The site demo's runner ends when its tests do (NOC-341): run 43828084330
// passed all of demo.e2e.test.mjs and then ran into the job's 30-minute
// limit. These tests hold the parts that can keep a test file's process
// alive to that: the site's server, the Chromium cdp.mjs starts, a call to
// a Chromium that went away, and a handle a test file leaves open.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { connect } from "node:net";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { launch } from "./cdp.mjs";
import { startServer } from "./server.mjs";
import { endsWhenDone } from "./teardown.mjs";

endsWhenDone();

/** Resolves to what `promise` resolves to, or rejects naming `what` after `ms`. */
const within = (ms, what, promise) => {
  let timer;
  return Promise.race([
    promise,
    new Promise((_, reject) => {
      timer = setTimeout(
        () => reject(new Error(`${what}: not within ${ms} ms`)),
        ms,
      );
    }),
  ]).finally(() => clearTimeout(timer));
};

/** Every process from /proc: its pid, state and parent; zombies left out. */
const processes = () =>
  readdirSync("/proc")
    .filter((entry) => /^\d+$/.test(entry))
    .map((pid) => {
      try {
        // After the command's closing parenthesis: state, then the parent.
        const stat = readFileSync(`/proc/${pid}/stat`, "utf8");
        const [state, ppid] = stat.slice(stat.lastIndexOf(")") + 2).split(" ");
        return { pid: Number(pid), state, ppid: Number(ppid) };
      } catch {
        return null;
      }
    })
    // A zombie holds no pipe, and the container's init may never reap it.
    .filter((p) => p !== null && p.state !== "Z");

/** The pids of `root` and every process it started, alive now. */
const tree = (root) => {
  const all = processes();
  const found = new Set([root]);
  for (let grew = true; grew; ) {
    grew = false;
    for (const p of all) {
      if (found.has(p.ppid) && !found.has(p.pid)) {
        found.add(p.pid);
        grew = true;
      }
    }
  }
  return [...found];
};

test("the server closes while a browser is in the middle of a request", {
  timeout: 10_000,
}, async () => {
  const server = await startServer({ port: 0, root: process.cwd() });
  const socket = connect(Number(new URL(server.url).port), "127.0.0.1");
  socket.on("error", () => {});
  await new Promise((resolve) => socket.once("connect", resolve));
  socket.write("GET / HTTP/1.1\r\nHost: localhost\r\n");
  await new Promise((resolve) => setTimeout(resolve, 100));
  try {
    await within(2000, "server.close()", server.close());
  } finally {
    socket.destroy();
  }
});

test("closing a Chromium that already went away returns", {
  timeout: 30_000,
}, async () => {
  const browser = await launch();
  process.kill(browser.pid, "SIGKILL");
  await new Promise((resolve) => setTimeout(resolve, 200));
  await within(5000, "close() after Chromium died", browser.close());
});

test("a call to a Chromium that goes away fails instead of waiting", {
  timeout: 30_000,
}, async () => {
  const browser = await launch();
  try {
    const tab = await browser.tab("about:blank");
    const call = tab.evaluate("new Promise(() => {})");
    await new Promise((resolve) => setTimeout(resolve, 200));
    process.kill(browser.pid, "SIGKILL");
    await within(
      5000,
      "a call in flight when Chromium died",
      assert.rejects(call, /Chromium/),
    );
  } finally {
    await within(5000, "close()", browser.close());
  }
});

test("close() ends every process Chromium started, so none holds its pipes open", {
  timeout: 30_000,
}, async () => {
  const browser = await launch();
  let started;
  try {
    await browser.tab("about:blank");
    started = tree(browser.pid);
    assert.ok(started.length > 1, "Chromium runs helper processes");
  } finally {
    await within(10_000, "close()", browser.close());
  }
  const live = new Set(processes().map((p) => p.pid));
  assert.deepEqual(
    started.filter((pid) => live.has(pid)),
    [],
    "a Chromium process is left",
  );
});

test("a test file that leaves a handle open ends soon after its tests, and names the handle", {
  timeout: 60_000,
}, async () => {
  const fixture = fileURLToPath(
    new URL("./fixtures/leaves-a-timer.mjs", import.meta.url),
  );
  // NODE_TEST_CONTEXT would make the child report to this runner.
  const { NODE_TEST_CONTEXT: _, ...env } = process.env;
  const child = spawn(process.execPath, ["--test", fixture], {
    env: { ...env, SITE_DEMO_LINGER_MS: "500" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  child.stdout.on("data", (chunk) => {
    output += chunk;
  });
  child.stderr.on("data", (chunk) => {
    output += chunk;
  });
  const code = await within(
    30_000,
    "the file with an open handle",
    new Promise((resolve) => child.on("exit", resolve)),
  ).catch((error) => {
    child.kill("SIGKILL");
    throw error;
  });
  assert.notEqual(code, 0, output);
  assert.match(output, /kept it running/);
  assert.match(output, /Timeout/);
});

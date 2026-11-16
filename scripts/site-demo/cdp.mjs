// A small driver for Chromium over the DevTools protocol, for the tests of
// tabs the visitor does not look at (NOC-319, a demo tab opened in the
// background). Playwright emulates focus and visibility on every page it
// drives, so its pages always read visible and focused; a tab this driver
// opens in the background is hidden, as in a visitor's browser, and a
// second window is visible but not focused. It runs the Chromium
// Playwright installs, in its own headless mode, with a fresh profile, so
// every browser starts with an empty OPFS.
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright";

/** How long Chromium has to say where it listens, or to answer a call. */
const PATIENCE_MS = 30_000;

/**
 * Starts Chromium; resolves to { tab, window, close, pid }. Chromium runs in
 * a process group of its own, and close() ends the whole group, so no
 * renderer is left holding the pipe this process reads, which would keep
 * the test file running after its tests (NOC-341).
 */
export const launch = async () => {
  const profile = mkdtempSync(join(tmpdir(), "site-demo-cdp-"));
  const browser = spawn(
    chromium.executablePath(),
    [
      "--headless=new",
      "--remote-debugging-port=0",
      `--user-data-dir=${profile}`,
      "--no-first-run",
      "--no-default-browser-check",
      "--no-sandbox",
      "about:blank",
    ],
    { stdio: ["ignore", "ignore", "pipe"], detached: true },
  );
  const exited = new Promise((resolve) => browser.once("exit", resolve));
  const running = () =>
    browser.exitCode === null && browser.signalCode === null;
  /** Sends `signal` to Chromium's process group, which may have ended. */
  const signal = (name) => {
    try {
      process.kill(-browser.pid, name);
    } catch {}
  };
  let socket;
  const pending = new Map();
  /** Fails every call still waiting for an answer. */
  const fail = (why) => {
    for (const call of pending.values()) call.reject(new Error(why));
    pending.clear();
  };
  let closing;
  const close = () => {
    closing ??= (async () => {
      fail("Chromium was closed");
      socket?.close();
      if (running()) {
        signal("SIGTERM");
        const timer = setTimeout(() => signal("SIGKILL"), 5000);
        await exited;
        clearTimeout(timer);
      }
      // Its renderers and helpers, should one outlive it.
      signal("SIGKILL");
      browser.stderr.destroy();
      rmSync(profile, { recursive: true, force: true });
    })();
    return closing;
  };
  exited.then((code) => fail(`Chromium ended (${code ?? browser.signalCode})`));

  try {
    const endpoint = await new Promise((resolve, reject) => {
      let out = "";
      const timer = setTimeout(
        () => reject(new Error(`Chromium did not start: ${out}`)),
        PATIENCE_MS,
      );
      browser.stderr.on("data", (chunk) => {
        out += chunk;
        const found = out.match(/ws:\/\/\S+/);
        if (found) {
          clearTimeout(timer);
          resolve(found[0]);
        }
      });
      exited.then((code) => {
        clearTimeout(timer);
        reject(new Error(`Chromium ended (${code}): ${out}`));
      });
    });
    socket = new WebSocket(endpoint);
    await new Promise((resolve, reject) => {
      socket.addEventListener("open", resolve, { once: true });
      socket.addEventListener("error", reject, { once: true });
    });
  } catch (error) {
    await close();
    throw error;
  }
  socket.addEventListener("close", () =>
    fail("Chromium's DevTools connection closed"),
  );
  let last = 0;
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    const call = pending.get(message.id);
    if (call === undefined) return;
    pending.delete(message.id);
    if (message.error) call.reject(new Error(JSON.stringify(message.error)));
    else call.resolve(message.result);
  });
  const send = (method, params = {}, sessionId = undefined) =>
    new Promise((resolve, reject) => {
      if (!running()) {
        reject(new Error(`Chromium ended before ${method}`));
        return;
      }
      last += 1;
      pending.set(last, { resolve, reject });
      socket.send(JSON.stringify({ id: last, method, params, sessionId }));
    });

  // Chromium says where it listens before its first window is open, and
  // until then it refuses a new tab ("no browser is open").
  const since = Date.now();
  for (;;) {
    const { targetInfos } = await send("Target.getTargets");
    if (targetInfos.some((target) => target.type === "page")) break;
    if (Date.now() - since > PATIENCE_MS) {
      await close();
      throw new Error("Chromium opened no window");
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }

  /** Opens `url` in a new tab, or a new window; `background` leaves it hidden behind the current tab. */
  const open = async (url, { background = false, window = false } = {}) => {
    const { targetId } = await send("Target.createTarget", {
      url,
      background,
      newWindow: window,
    });
    const { sessionId } = await send("Target.attachToTarget", {
      targetId,
      flatten: true,
    });
    /** The value of `expression` in the page, awaited. */
    const evaluate = async (expression) => {
      const result = await send(
        "Runtime.evaluate",
        { expression, awaitPromise: true, returnByValue: true },
        sessionId,
      );
      if (result.exceptionDetails) {
        throw new Error(JSON.stringify(result.exceptionDetails));
      }
      return result.result.value;
    };
    /** Brings the tab to the front of its window, which hides the others. */
    const front = () => send("Target.activateTarget", { targetId });
    return { evaluate, front };
  };

  return {
    tab: (url, options) => open(url, options),
    window: (url) => open(url, { window: true }),
    close,
    pid: browser.pid,
  };
};

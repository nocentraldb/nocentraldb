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

/** Starts Chromium; resolves to { tab, window, close }. */
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
    { stdio: ["ignore", "ignore", "pipe"] },
  );
  const endpoint = await new Promise((resolve, reject) => {
    let out = "";
    browser.stderr.on("data", (chunk) => {
      out += chunk;
      const found = out.match(/ws:\/\/\S+/);
      if (found) resolve(found[0]);
    });
    browser.on("exit", (code) =>
      reject(new Error(`Chromium ended (${code}): ${out}`)),
    );
  });
  const socket = new WebSocket(endpoint);
  await new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", reject, { once: true });
  });
  let last = 0;
  const pending = new Map();
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
      last += 1;
      pending.set(last, { resolve, reject });
      socket.send(JSON.stringify({ id: last, method, params, sessionId }));
    });

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
    close: async () => {
      socket.close();
      const ended = new Promise((resolve) => browser.once("exit", resolve));
      browser.kill();
      await ended;
      rmSync(profile, { recursive: true, force: true });
    },
  };
};

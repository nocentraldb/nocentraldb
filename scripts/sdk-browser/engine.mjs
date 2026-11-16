// The browser engine the TypeScript SDK's browser tests run in (NOC-344):
// SDK_BROWSER_ENGINE names it, Chromium when it names none. The workflow
// browser-view.yml runs every e2e file once per engine, one step each, so
// a failing engine names itself in its step. Every test runs in all three;
// an engine that cannot run one would skip it with the reason in the test,
// and docs/getting-started-browser.md would list the gap. Where an engine
// has a documented limit, the test asserts the limit there instead, as
// frozen.e2e.test.mjs does for WebKit (NOC-352).
//
//   SDK_BROWSER_ENGINE=firefox node --test 'scripts/sdk-browser/*.test.mjs'
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium, firefox, webkit } from "playwright";

const TYPES = { chromium, firefox, webkit };

/**
 * What each engine launches with. Firefox asks the person before
 * `navigator.storage.persist()` resolves, and a headless Firefox shows the
 * prompt to no one, so `requestPersistent()` would wait forever: its
 * testing prefs answer the prompt with "allow", as a person might.
 */
const OPTIONS = {
  chromium: {},
  firefox: {
    firefoxUserPrefs: {
      "dom.storageManager.prompt.testing": true,
      "dom.storageManager.prompt.testing.allow": true,
    },
  },
  webkit: {},
};

export const ENGINES = Object.keys(TYPES);

/** The engine `env` names; throws on one Playwright does not have. */
export const engineFrom = (env) => {
  const name = env.SDK_BROWSER_ENGINE || "chromium";
  if (!ENGINES.includes(name)) {
    throw new Error(
      `SDK_BROWSER_ENGINE=${name} is no engine: use one of ${ENGINES.join(", ")}`,
    );
  }
  return name;
};

/** The engine of this run. */
export const engine = engineFrom(process.env);

/**
 * What each engine's private window launches with (NOC-347). In WebKit
 * every `browser.newContext()` is one, an ephemeral context; in Chromium
 * it is an off-the-record context, as Incognito is. Firefox's contexts
 * are normal windows, so its prefs start the browser in private browsing,
 * where every window is private.
 */
const PRIVATE = {
  chromium: {},
  firefox: {
    firefoxUserPrefs: {
      ...OPTIONS.firefox.firefoxUserPrefs,
      "browser.privatebrowsing.autostart": true,
    },
  },
  webkit: {},
};

/**
 * This run's engine with private windows only: `newContext()` opens a
 * private window, and `close()` closes the browser. Only private.e2e.test.mjs
 * uses it; every other file runs in normal windows, from `launch`.
 */
export const launchPrivate = async () => {
  const browser = await TYPES[engine].launch(PRIVATE[engine]);
  return {
    newContext: () => browser.newContext(),
    close: () => browser.close(),
  };
};

/**
 * This run's engine, as the e2e files use a browser: `newContext()` opens
 * a normal window on a fresh profile, with an OPFS of its own, and `close()`
 * closes every window it opened and removes the profiles. Not
 * `browser.newContext()`: in WebKit that is a private window, which has no
 * OPFS, so the SDK keeps the database in memory and loses it when its
 * owning tab closes or is hidden (private.e2e.test.mjs tests that case on
 * its own, from `launchPrivate`).
 */
export const launch = async () => {
  const open = new Set();
  return {
    newContext: async () => {
      const dir = await mkdtemp(join(tmpdir(), "ncdb-sdk-browser-"));
      const context = await TYPES[engine].launchPersistentContext(
        dir,
        OPTIONS[engine],
      );
      const close = context.close.bind(context);
      const window = { context, dir };
      open.add(window);
      context.close = async () => {
        open.delete(window);
        await close();
        await rm(dir, { recursive: true, force: true });
      };
      return context;
    },
    close: async () => {
      for (const { context } of [...open]) await context.close();
    },
  };
};

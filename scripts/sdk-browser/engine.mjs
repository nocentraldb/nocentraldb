// The browser engine the TypeScript SDK's browser tests run in (NOC-344):
// SDK_BROWSER_ENGINE names it, Chromium when it names none. The workflow
// browser-view.yml runs every e2e file once per engine, one step each, so
// a failing engine names itself in its step. An engine that cannot run a
// test skips it there with `skipIn`, and the reason stands in the test; the
// gaps are listed in docs/getting-started-browser.md ("Browsers").
//
//   SDK_BROWSER_ENGINE=firefox node --test 'scripts/sdk-browser/*.test.mjs'
import { chromium, firefox, webkit } from "playwright";

const TYPES = { chromium, firefox, webkit };

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

/** Launches this run's engine. */
export const launch = () => TYPES[engine].launch();

/** The `skip` option of a test that `engine` cannot run: the reason `gaps` gives for it, or false. */
export const skipIn = (gaps, name = engine) =>
  name in gaps ? `${name}: ${gaps[name]}` : false;

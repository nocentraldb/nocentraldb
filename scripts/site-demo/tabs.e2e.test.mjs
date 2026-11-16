// The landing page's demo in tabs the visitor does not look at (NOC-319,
// the owner's report on #333): a task added in one tab shows in another
// tab of the page within a second, whether that tab is hidden behind the
// first, opened in the background and never shown yet, or in a second
// window that is visible but not focused. Chromium through the DevTools
// protocol (cdp.mjs), since Playwright's pages always read visible and
// focused. Not part of `pnpm gates`; `repo:site-demo` and the workflow
// browser-view.yml run it, as demo.e2e.test.mjs.
import assert from "node:assert/strict";
import { after, before } from "node:test";
import { build } from "./build.mjs";
import { launch } from "./cdp.mjs";
import { startServer } from "./server.mjs";
import { endsWhenDone, test } from "./teardown.mjs";

endsWhenDone();

let server;

before(async () => {
  build();
  server = await startServer({ port: 0 });
});

after(async () => {
  await server?.close();
});

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** What a tab shows: its visibility, focus, the demo's state and its open tasks. */
const seen = async (tab) =>
  JSON.parse(
    await tab.evaluate(`JSON.stringify({
      visibility: document.visibilityState,
      focused: document.hasFocus(),
      demo: document.querySelector("[data-demo-status]")?.dataset.state,
      open: [...document.querySelectorAll("#demo-open li .title")].map((title) => title.textContent),
    })`),
  );

/** Waits until `check` holds for what `tab` shows; what it shows then, or the last thing it showed. */
const until = async (tab, check, ms) => {
  const start = Date.now();
  let last = await seen(tab);
  while (!check(last) && Date.now() - start < ms) {
    await sleep(50);
    last = await seen(tab);
  }
  return last;
};

/** Waits until the demo in `tab` runs. */
const ready = (tab, ms = 60_000) =>
  until(tab, (shown) => shown.demo === "ready", ms);

/** Adds a task through the demo's form in `tab`. */
const add = (tab, title) =>
  tab.evaluate(`(() => {
    const input = document.querySelector("#demo-add input[name=title]");
    input.value = ${JSON.stringify(title)};
    document.querySelector("#demo-add").requestSubmit();
    return true;
  })()`);

/** Whether `title` is among the open tasks `tab` shows within `ms`, and what it shows then. */
const shows = async (tab, title, ms = 1000) => {
  const shown = await until(tab, (s) => s.open.includes(title), ms);
  return { listed: shown.open.includes(title), shown };
};

test("a tab opened in the background runs the demo and shows a task added in the other tab, while still hidden", async () => {
  const browser = await launch();
  try {
    const url = `${server.url}/#demo`;
    const first = await browser.tab(url);
    assert.equal((await ready(first)).demo, "ready");
    const second = await browser.tab(url, { background: true });
    const started = await ready(second, 30_000);
    assert.equal(started.visibility, "hidden");
    assert.equal(started.demo, "ready", "the hidden tab started the demo");
    await add(first, "Added while the other tab was never shown");
    const { listed, shown } = await shows(
      second,
      "Added while the other tab was never shown",
    );
    assert.ok(listed, `the hidden tab lists it: ${JSON.stringify(shown)}`);
    assert.equal(shown.visibility, "hidden");
  } finally {
    await browser.close();
  }
});

test("a tab hidden after the demo started shows the other tab's tasks, and the other tab shows its own", async () => {
  const browser = await launch();
  try {
    const url = `${server.url}/#demo`;
    const first = await browser.tab(url);
    await ready(first);
    const second = await browser.tab(url);
    await second.front();
    assert.equal((await ready(second)).demo, "ready");
    await first.front();
    await add(first, "Added while the second tab was hidden");
    const hidden = await shows(second, "Added while the second tab was hidden");
    assert.ok(hidden.listed, JSON.stringify(hidden.shown));
    assert.equal(hidden.shown.visibility, "hidden");
    // The first tab owns the database; now it is the hidden one.
    await second.front();
    await add(second, "Added while the owning tab was hidden");
    const owner = await shows(first, "Added while the owning tab was hidden");
    assert.ok(owner.listed, JSON.stringify(owner.shown));
    assert.equal(owner.shown.visibility, "hidden");
  } finally {
    await browser.close();
  }
});

test("a second window, visible but not focused, shows the first window's tasks, and the first shows its own", async () => {
  const browser = await launch();
  try {
    const url = `${server.url}/#demo`;
    const first = await browser.tab(url);
    await ready(first);
    const second = await browser.window(url);
    await ready(second);
    await first.front();
    await add(first, "Added in the focused window");
    const unfocused = await shows(second, "Added in the focused window");
    assert.ok(unfocused.listed, JSON.stringify(unfocused.shown));
    assert.equal(unfocused.shown.visibility, "visible");
    await add(second, "Added in the unfocused window");
    const focused = await shows(first, "Added in the unfocused window");
    assert.ok(focused.listed, JSON.stringify(focused.shown));
  } finally {
    await browser.close();
  }
});

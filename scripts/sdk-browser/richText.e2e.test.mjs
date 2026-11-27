// End to end (NOC-461, 4c): `space.richText` through the real worker, in
// Chromium. The page loads a copy of the package whose core holds rich
// text (richTextCopy.mjs), with Yjs, an optional peer of the SDK, mapped
// by an import map onto the repository's node_modules, as an app's bundler
// would resolve it. A binding opens on a note's body, writes what is typed
// into it, and a binding opened after the database was opened again reads
// it back; the package's own core, without rich text, answers
// `notSupported`. Run as sdk.e2e.test.mjs is: by the task
// `repo:sdk-browser`, not by `pnpm gates`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { after, before, test } from "node:test";
import { build, DIST, LOCK, prebuilt, ROOT } from "./build.mjs";
import { launch } from "./engine.mjs";
import {
  buildRichText,
  prebuiltRichText,
  RICH_TEXT_DIST,
} from "./richTextCopy.mjs";
import { startServer } from "./server.mjs";

let richTextServer;
let plainServer;
let browser;

before(async () => {
  if (process.env.SDK_BROWSER_PREBUILT === "1") {
    prebuilt();
    prebuiltRichText();
  } else {
    build();
    buildRichText();
  }
  richTextServer = await startServer({ port: 0, dist: RICH_TEXT_DIST });
  plainServer = await startServer({ port: 0, dist: DIST });
  browser = await launch();
});

after(async () => {
  await browser?.close();
  await richTextServer?.close();
  await plainServer?.close();
});

/** The test lock, with a rich-text `body` on tasks and its vocabulary. */
const lock = (() => {
  const base = JSON.parse(readFileSync(LOCK, "utf8"));
  base.collections.tasks.fields.body = {
    rule: 1,
    since: 3,
    type: "ncdb/richText",
    vocabulary: { marks: [], nodes: ["paragraph"] },
  };
  return base;
})();

const MODEL = `nc.model({
  tasks: nc.collection({
    title: nc.string(),
    body: nc.richText({ vocabulary: { nodes: ["paragraph"], marks: [] } }),
  }),
})`;

// Yjs and the lib0 modules it imports, as files of the repository's
// node_modules: the import map's entries, and where each is served from.
const require = createRequire(join(ROOT, "packages", "sdk", "package.json"));
const modules = (() => {
  const served = new Map();
  const entries = {};
  const packageDir = (name, from) =>
    dirname(
      createRequire(from).resolve(`${name}/package.json`, {
        paths: [dirname(from)],
      }),
    );
  /** Serves the file at `path` under `url`, and each module it imports. */
  const file = (path, url) => {
    if (served.has(url)) return;
    served.set(url, path);
    for (const [, specifier] of readFileSync(path, "utf8").matchAll(
      /(?:\bfrom|\bimport)\s*['"]([^'"]+)['"]/g,
    )) {
      if (specifier.startsWith(".")) {
        file(
          join(dirname(path), specifier),
          new URL(specifier, `http://x${url}`).pathname,
        );
      } else {
        bare(specifier, path);
      }
    }
  };
  /** Maps the bare `specifier`, as `from` imports it, onto its package's file. */
  const bare = (specifier, from) => {
    if (entries[specifier]) return;
    const [scope, ...rest] = specifier.split("/");
    const dir = packageDir(scope, from);
    const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
    const key = rest.length ? `./${rest.join("/")}` : ".";
    // Conditions as a browser's bundler picks them.
    const pick = (entry) =>
      typeof entry === "string" || entry === undefined
        ? entry
        : pick(entry.browser ?? entry.import ?? entry.module ?? entry.default);
    const target = pick(pkg.exports?.[key]) ?? pkg.module;
    const url = `/modules/${scope}/${target.replace(/^\.\//, "")}`;
    entries[specifier] = url;
    file(join(dir, target), url);
  };
  bare("yjs", require.resolve("yjs"));
  return { served, entries };
})();

const page = `<!doctype html>
<meta charset="utf-8">
<script type="importmap">${JSON.stringify({ imports: modules.entries })}</script>
<script type="module">
  import { nc } from "/sdk/index.js";
  import * as Y from "yjs";
  window.nc = nc;
  window.Y = Y;
  window.ready = true;
</script>`;

const open = async (server) => {
  const tab = await (await browser.newContext()).newPage();
  const errors = [];
  tab.on("pageerror", (error) => errors.push(error));
  await tab.route(`${server.url}/rich-text.html`, (route) =>
    route.fulfill({ body: page, contentType: "text/html; charset=utf-8" }),
  );
  await tab.route(`${server.url}/modules/**`, (route) => {
    const path = modules.served.get(new URL(route.request().url()).pathname);
    if (!path) return route.fulfill({ status: 404 });
    return route.fulfill({
      body: readFileSync(path),
      contentType: "text/javascript; charset=utf-8",
    });
  });
  tab.on(
    "console",
    (m) => process.env.RT_DEBUG && console.log("console:", m.text()),
  );
  tab.on(
    "requestfailed",
    (r) => process.env.RT_DEBUG && console.log("failed:", r.url()),
  );
  tab.on(
    "response",
    (r) =>
      process.env.RT_DEBUG &&
      r.status() >= 400 &&
      console.log("status:", r.status(), r.url()),
  );
  await tab.goto(`${server.url}/rich-text.html`);
  await tab.waitForFunction(() => window.ready === true);
  return { tab, errors };
};

test("a binding writes what is typed, and a binding after a reopen reads it back", async () => {
  const { tab, errors } = await open(richTextServer);
  const result = await tab.evaluate(
    async ({ lock, MODEL }) => {
      const { nc, Y } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "rich" });
      const space = await db.spaces.create();
      const { ids } = await space.create(model.tasks, { title: "Note" });
      const body = await space.richText(model.tasks, ids[0], "body");
      const root = body.doc.getXmlFragment("default");
      const paragraph = new Y.XmlElement("paragraph");
      paragraph.insert(0, [new Y.XmlText("Typed in the browser")]);
      root.insert(0, [paragraph]);
      const status = body.status;
      await body.close();
      await db.close();
      const again = await nc.open({ model, lock, name: "rich" });
      const read = await (await again.spaces.get(space.id)).richText(
        model.tasks,
        ids[0],
        "body",
      );
      const text = read.doc.getXmlFragment("default").toString();
      await read.close();
      await again.close();
      return { status, text };
    },
    { lock, MODEL },
  );
  assert.deepEqual(errors, []);
  assert.deepEqual(result.status, { status: "shown" });
  assert.equal(result.text, "<paragraph>Typed in the browser</paragraph>");
});

test("the package's own core, without rich text, answers notSupported", async () => {
  const { tab, errors } = await open(plainServer);
  const code = await tab.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "plain" });
      const space = await db.spaces.create();
      const { ids } = await space.create(model.tasks, { title: "Note" });
      try {
        await space.richText(model.tasks, ids[0], "body");
        return "opened";
      } catch (error) {
        return error instanceof nc.Error ? error.code : String(error);
      } finally {
        await db.close();
      }
    },
    { lock, MODEL },
  );
  assert.deepEqual(errors, []);
  assert.equal(code, "notSupported");
});

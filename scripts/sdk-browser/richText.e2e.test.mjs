// End to end (NOC-461, 4c): `space.richText` through the real worker, in
// Chromium. The page loads a copy of the package whose core holds rich
// text (richTextCopy.mjs), with Yjs, an optional peer of the SDK, mapped
// by an import map onto the repository's node_modules, as an app's bundler
// would resolve it. A binding opens on a note's body, writes what is typed
// into it, and a binding opened after the database was opened again reads
// it back; a read of the object gives the field's Markdown (NOC-589);
// `nc.richText.fromMarkdown` writes the shared table's first documents
// (NOC-578); the package's own core, without rich text, answers
// `notSupported`, to `space.richText` and to a write of a first document
// (NOC-590 2a). Run as sdk.e2e.test.mjs is: by the task
// `repo:sdk-browser`, not by `pnpm gates`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { build, DIST, LOCK, prebuilt, ROOT } from "./build.mjs";
import { launch } from "./engine.mjs";
import { routeModules, yjsModules } from "./importMap.mjs";
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

const page = `<!doctype html>
<meta charset="utf-8">
<script type="importmap">${JSON.stringify({ imports: yjsModules.entries })}</script>
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
  // Yjs draws a new client id, and says so, when an update it applies
  // holds items of its own; the binding's writes would then be under an id
  // no lease holds.
  tab.on("console", (message) => {
    if (message.text().includes("Changed the client-id")) {
      errors.push(message.text());
    }
  });
  await tab.route(`${server.url}/rich-text.html`, (route) =>
    route.fulfill({ body: page, contentType: "text/html; charset=utf-8" }),
  );
  await routeModules(tab, server.url);
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

// A read of the object gives a rich-text field's Markdown, a string, and a
// field nothing was written to reads null, as any unset field (RFC 0013,
// "Reading"; NOC-589).
test("a read gives the Markdown a binding wrote, and null before anything was written", async () => {
  const { tab, errors } = await open(richTextServer);
  const result = await tab.evaluate(
    async ({ lock, MODEL }) => {
      const { nc, Y } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "reads" });
      const space = await db.spaces.create();
      const { ids } = await space.create(model.tasks, { title: "Note" });
      const before = (await space.get(model.tasks, ids[0])).body;
      const body = await space.richText(model.tasks, ids[0], "body");
      const paragraph = new Y.XmlElement("paragraph");
      paragraph.insert(0, [new Y.XmlText("Typed *here*")]);
      body.doc.getXmlFragment("default").insert(0, [paragraph]);
      await body.close();
      const after = (await space.get(model.tasks, ids[0])).body;
      await db.close();
      return { before, after };
    },
    { lock, MODEL },
  );
  assert.deepEqual(errors, []);
  assert.equal(result.before, null);
  assert.equal(result.after, "Typed \\*here\\*");
});

/** The table of first documents every SDK runs (NOC-463). */
const TABLE = JSON.parse(
  readFileSync(
    join(ROOT, "crates/ncdb-core/tests/vectors/rich-text/from-markdown.json"),
    "utf8",
  ),
);

// nc.richText.fromMarkdown writes a first document through the core's
// read-back, the path the Rust SDK's RichText::from_markdown takes, so
// every case of the shared table reads as the Markdown it names, and every
// refused case is refused with invalidValue (RFC 0013, "The first
// content"; NOC-578).
test("every case of the shared table is written with fromMarkdown and reads as its Markdown", async () => {
  const { tab, errors } = await open(richTextServer);
  const result = await tab.evaluate(
    async ({ lock, MODEL, TABLE }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "table" });
      const space = await db.spaces.create();
      const reads = [];
      for (const { name, markdown } of TABLE.cases) {
        const { ids } = await space.create(model.tasks, {
          body: nc.richText.fromMarkdown(markdown),
        });
        reads.push([name, (await space.get(model.tasks, ids[0])).body]);
      }
      const refused = [];
      for (const { name, markdown } of TABLE.refused) {
        try {
          await space.create(model.tasks, {
            title: "a",
            body: nc.richText.fromMarkdown(markdown),
          });
          refused.push([name, "written"]);
        } catch (error) {
          refused.push([
            name,
            error instanceof nc.Error ? error.code : String(error),
          ]);
        }
      }
      await db.close();
      return { reads, refused };
    },
    { lock, MODEL, TABLE },
  );
  assert.deepEqual(errors, []);
  assert.deepEqual(
    result.reads,
    TABLE.cases.map(({ name, reads }) => [name, reads]),
  );
  assert.deepEqual(
    result.refused,
    TABLE.refused.map(({ name }) => [name, "invalidValue"]),
  );
});

test("fromMarkdown into a field that holds content is refused with richTextNotEmpty", async () => {
  const { tab, errors } = await open(richTextServer);
  const result = await tab.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "not-empty" });
      const space = await db.spaces.create();
      const { ids } = await space.create(model.tasks, { title: "Note" });
      await space.update(model.tasks, ids[0], {
        body: nc.richText.fromMarkdown("First."),
      });
      let code = "written";
      try {
        await space.update(model.tasks, ids[0], {
          body: nc.richText.fromMarkdown("Second."),
        });
      } catch (error) {
        code = error instanceof nc.Error ? error.code : String(error);
      }
      const body = (await space.get(model.tasks, ids[0])).body;
      await db.close();
      return { code, body };
    },
    { lock, MODEL },
  );
  assert.deepEqual(errors, []);
  assert.deepEqual(result, { code: "richTextNotEmpty", body: "First." });
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

test("the package's own core, without rich text, refuses a first document with notSupported", async () => {
  const { tab, errors } = await open(plainServer);
  const code = await tab.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "plain-first" });
      const space = await db.spaces.create();
      try {
        await space.create(model.tasks, {
          title: "Note",
          body: nc.richText.fromMarkdown("First."),
        });
        return "written";
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

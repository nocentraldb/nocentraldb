// End to end (NOC-461, 4c-2): RFC 0013's three measures with TipTap,
// through the real worker, in the engine SDK_BROWSER_ENGINE names
// (Chromium, Firefox or WebKit; CI runs all three). The page loads the
// copy of the package whose core holds rich text, TipTap and
// @nocentraldb/tiptap bundled with esbuild, and Yjs through the import map.
// In one database, a second binding on the same field stands in for a
// newer app, which writes a node the older app's editor lacks. Run as
// sdk.e2e.test.mjs is: by the task `repo:sdk-browser`, not by
// `pnpm gates`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { build, LOCK, prebuilt } from "./build.mjs";
import { launch } from "./engine.mjs";
import { routeModules, yjsModules } from "./importMap.mjs";
import {
  buildRichText,
  prebuiltRichText,
  RICH_TEXT_DIST,
} from "./richTextCopy.mjs";
import { startServer } from "./server.mjs";
import { BUNDLE, bundle } from "./tiptap/bundle.mjs";

let server;
let browser;

before(async () => {
  if (process.env.SDK_BROWSER_PREBUILT === "1") {
    prebuilt();
    prebuiltRichText();
  } else {
    build();
    buildRichText();
  }
  await bundle();
  server = await startServer({ port: 0, dist: RICH_TEXT_DIST });
  browser = await launch();
});

after(async () => {
  await browser?.close();
  await server?.close();
});

const page = `<!doctype html>
<meta charset="utf-8">
<script type="importmap">${JSON.stringify({ imports: yjsModules.entries })}</script>
<div id="older"></div>
<script type="module" src="/tiptap.js"></script>`;

const open = async () => {
  const tab = await (await browser.newContext()).newPage();
  const errors = [];
  tab.on("pageerror", (error) => errors.push(error));
  tab.on("console", (message) => {
    if (message.text().includes("Changed the client-id")) {
      errors.push(message.text());
    }
  });
  await tab.route(`${server.url}/tiptap.html`, (route) =>
    route.fulfill({ body: page, contentType: "text/html; charset=utf-8" }),
  );
  await tab.route(`${server.url}/tiptap.js`, (route) =>
    route.fulfill({
      body: readFileSync(BUNDLE),
      contentType: "text/javascript; charset=utf-8",
    }),
  );
  await routeModules(tab, server.url);
  await tab.goto(`${server.url}/tiptap.html`);
  await tab.waitForFunction(() => window.ready === true);
  return { tab, errors };
};

/**
 * In the page: a database whose `tasks.body` declares StarterKit's
 * vocabulary, a task, and two bindings on its body, `newer` and `older`.
 * Defines window.scene with them and a few helpers.
 */
const setUp = async (tab, name) =>
  tab.evaluate(
    async ({ base, name }) => {
      const { nc, getSchema, StarterKit, Y } = window.tt;
      const kit = [StarterKit.configure({ undoRedo: false })];
      const schema = getSchema(kit);
      const vocabulary = {
        nodes: Object.keys(schema.nodes)
          .filter((n) => n !== "doc" && n !== "text")
          .sort(),
        marks: Object.keys(schema.marks).sort(),
      };
      const lock = structuredClone(base);
      lock.collections.tasks.fields.body = {
        rule: 1,
        since: 3,
        type: "ncdb/richText",
        vocabulary,
      };
      const model = nc.model({
        tasks: nc.collection({
          title: nc.string(),
          body: nc.richText({ vocabulary }),
        }),
      });
      const db = await nc.open({ model, lock, name });
      const space = await db.spaces.create();
      const { ids } = await space.create(model.tasks, { title: "Note" });
      const open = () => space.richText(model.tasks, ids[0], "body");
      const xml = async () => {
        const reader = await open();
        const text = reader.doc.getXmlFragment("default").toString();
        await reader.close();
        return text;
      };
      const paragraph = (words) => {
        const p = new Y.XmlElement("paragraph");
        p.insert(0, [new Y.XmlText(words)]);
        return p;
      };
      /** The newer app writes a callout, which StarterKit lacks, and closes, which writes it. */
      const newerWritesCallout = async () => {
        const newer = await open();
        const callout = new Y.XmlElement("callout");
        callout.setAttribute("tone", "warn");
        callout.insert(0, [paragraph("Careful.")]);
        newer.doc.getXmlFragment("default").insert(0, [callout]);
        await newer.close();
      };
      const until = async (check) => {
        for (let i = 0; i < 200; i++) {
          if (await check()) return true;
          await new Promise((r) => setTimeout(r, 25));
        }
        return false;
      };
      window.scene = {
        db,
        kit,
        open,
        xml,
        paragraph,
        newerWritesCallout,
        until,
      };
    },
    { base: JSON.parse(readFileSync(LOCK, "utf8")), name },
  );

test("widening: an older editor opens a newer document, loses nothing and stays editable", async () => {
  const { tab, errors } = await open();
  await setUp(tab, "widening");
  const result = await tab.evaluate(async () => {
    const { ncEditor } = window.tt;
    const { kit, open, xml, newerWritesCallout, until, db } = window.scene;
    await newerWritesCallout();
    const older = await open();
    let editor;
    const mounted = ncEditor(older, {
      element: document.getElementById("older"),
      extensions: kit,
      onEditor: (e) => {
        editor = e;
      },
    });
    editor.commands.insertContentAt(
      editor.state.doc.content.size,
      "<p>Typed by the older app.</p>",
    );
    await older.close();
    mounted.destroy();
    const saved = await xml();
    const kept = await until(async () => true);
    await db.close();
    return { saved, kept, status: older.status };
  });
  assert.deepEqual(errors, []);
  assert.match(
    result.saved,
    /<callout tone="warn"><paragraph>Careful\.<\/paragraph><\/callout>/,
  );
  assert.match(result.saved, /Typed by the older app\./);
});

test("content check: a node that arrives while the editor is open rebuilds it, and typing goes on", async () => {
  const { tab, errors } = await open();
  await setUp(tab, "content-check");
  const result = await tab.evaluate(async () => {
    const { ncEditor } = window.tt;
    const { kit, open, xml, newerWritesCallout, until, db } = window.scene;
    const older = await open();
    const editors = [];
    const mounted = ncEditor(older, {
      element: document.getElementById("older"),
      extensions: kit,
      onEditor: (e) => editors.push(e),
    });
    await newerWritesCallout();
    const rebuilt = await until(() => editors.length === 2);
    const editor = editors.at(-1);
    editor.commands.insertContentAt(
      editor.state.doc.content.size,
      "<p>After the rebuild.</p>",
    );
    await older.close();
    mounted.destroy();
    const saved = await xml();
    await db.close();
    return { rebuilt, saved, firstDestroyed: editors[0].isDestroyed };
  });
  assert.deepEqual(errors, []);
  assert.equal(result.rebuilt, true);
  assert.equal(result.firstDestroyed, true);
  assert.match(result.saved, /<callout tone="warn">/);
  assert.match(result.saved, /After the rebuild\./);
});

test("guard: an editor without the measures deletes the unknown node, and that is not written; a deliberate deletion is", async () => {
  const { tab, errors } = await open();
  await setUp(tab, "guard");
  const result = await tab.evaluate(async () => {
    const { Editor, Collaboration, ncEditor } = window.tt;
    const { kit, open, xml, newerWritesCallout, until, db } = window.scene;
    await newerWritesCallout();
    // An app that leaves out the measures: TipTap deletes what its schema
    // lacks as it opens the document (RFC 0013, F16).
    const careless = await open();
    const raw = new Editor({
      element: document.getElementById("older"),
      extensions: [...kit, Collaboration.configure({ document: careless.doc })],
    });
    raw.commands.insertContentAt(1, "x");
    await until(() => careless.status.status !== "shown");
    const refused = careless.status;
    raw.destroy();
    await careless.close();
    const afterRefusal = await xml();
    // ncEditor's placeholder, deleted on purpose, is written.
    const older = await open();
    let editor;
    const mounted = ncEditor(older, {
      element: document.getElementById("older"),
      extensions: kit,
      onEditor: (e) => {
        editor = e;
      },
    });
    // Something stays: y-sync never writes a document emptied before its
    // first local edit, which it takes for one not yet filled.
    editor.commands.insertContentAt(0, "<p>Kept.</p>");
    let at = -1;
    editor.state.doc.forEach((node, offset) => {
      if (node.type.name === "callout") at = offset;
    });
    editor.commands.deleteRange({
      from: at,
      to: at + editor.state.doc.nodeAt(at).nodeSize,
    });
    await older.close();
    mounted.destroy();
    const afterDeletion = await xml();
    await db.close();
    return { refused, afterRefusal, afterDeletion };
  });
  assert.deepEqual(errors, []);
  assert.equal(result.refused.status, "notFullyShown");
  assert.match(result.refused.reason, /callout/);
  assert.match(result.afterRefusal, /<callout tone="warn">/);
  assert.doesNotMatch(result.afterDeletion, /callout/);
  assert.match(result.afterDeletion, /Kept\./);
});

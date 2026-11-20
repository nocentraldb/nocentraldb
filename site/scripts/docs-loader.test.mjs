// The docs collection's loader (NOC-361): Starlight's pages and the docs
// rendered from docs/. Astro keeps a page's rendered HTML until its file
// changes, but the npm line in that HTML follows release-notes/index.json, so
// a new npm line renders every page again.
import assert from "node:assert/strict";
import { test } from "node:test";
import { docsWithRepoLoader } from "../src/docs-loader.mjs";
import { REPO_DOCS } from "../src/repo-docs.mjs";

/** A loader context whose store and meta live in maps. */
const context = () => {
  const entries = new Map();
  const meta = new Map();
  return {
    entries,
    config: { root: new URL("../", import.meta.url) },
    store: {
      set: (entry) => entries.set(entry.id, entry),
      clear: () => entries.clear(),
      keys: () => [...entries.keys()],
    },
    meta: {
      get: (key) => meta.get(key),
      set: (key, value) => meta.set(key, value),
    },
    parseData: async ({ data }) => data,
    renderMarkdown: async (body) => ({ html: body }),
    generateDigest: (body) => String(body.length),
  };
};

/** A loader whose Starlight part only records whether the store was empty. */
const loader = (npm, seen) =>
  docsWithRepoLoader({
    starlight: {
      load: async (ctx) => {
        seen.push(ctx.store.keys().includes("docs/cached"));
        ctx.store.set({ id: "docs/cached" });
      },
    },
    npmLine: () => npm,
  });

test("the loader adds every rendered doc to Starlight's pages", async () => {
  const ctx = context();
  await loader("npm install @nocentraldb/sdk@alpha", []).load(ctx);
  for (const doc of REPO_DOCS) assert.ok(ctx.entries.has(doc.slug), doc.slug);
  assert.ok(ctx.entries.has("docs/cached"));
});

test("the same npm line keeps the rendered pages", async () => {
  const ctx = context();
  const seen = [];
  await loader("npm install @nocentraldb/sdk@alpha", seen).load(ctx);
  await loader("npm install @nocentraldb/sdk@alpha", seen).load(ctx);
  assert.deepEqual(seen, [false, true]);
});

test("a new npm line renders every page again", async () => {
  const ctx = context();
  const seen = [];
  await loader("npm install @nocentraldb/sdk@alpha", seen).load(ctx);
  await loader("npm install @nocentraldb/sdk@beta", seen).load(ctx);
  assert.deepEqual(seen, [false, false]);
});

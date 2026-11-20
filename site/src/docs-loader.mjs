// Starlight's docs collection, plus the developer docs rendered from their
// files in docs/ (NOC-361; which files and how: repo-docs.mjs) and the
// TypeScript API reference, rendered from the SDK's API report
// (api-reference.mjs).
import { readFileSync } from "node:fs";
import { docsLoader } from "@astrojs/starlight/loaders";
import { API_REFERENCE, apiReferenceEntry } from "./api-reference.mjs";
import { currentInstallLines } from "./install.mjs";
import { REPO_DOCS, repoDocEntry } from "./repo-docs.mjs";

/**
 * The docs loader. `starlight` loads Starlight's own pages; `npmLine` gives
 * the npm line the pages show for the repository at a root (install.mjs).
 */
export const docsWithRepoLoader = ({
  starlight = docsLoader(),
  npmLine = (root) => currentInstallLines(root).npm,
} = {}) => ({
  name: "ncdb-docs-loader",
  async load(context) {
    const root = new URL("../", context.config.root);
    // Astro keeps a page's rendered HTML until its file changes; the npm
    // line in it follows release-notes/index.json, so a new one renders
    // every page again.
    const npm = npmLine(root);
    if (context.meta.get("npm") !== npm) {
      context.store.clear();
      context.meta.set("npm", npm);
    }
    // Starlight's own pages first: its glob drops entries it did not load.
    await starlight.load(context);
    await addRepoDocs(context, root);
    // In `astro dev`, an edit of a rendered doc renders it again.
    const files = [
      ...REPO_DOCS.map((doc) => `docs/${doc.file}`),
      API_REFERENCE.file,
    ].map((file) => new URL(file, root).pathname);
    context.watcher?.add(files);
    context.watcher?.on("change", (path) => {
      if (files.includes(path)) addRepoDocs(context, root);
    });
  },
});

/** Adds every page rendered from a file of the repository at `root`. */
const addRepoDocs = async (context, root) => {
  const read = (file) => readFileSync(new URL(file, root), "utf8");
  for (const doc of REPO_DOCS)
    await add(
      context,
      doc.slug,
      repoDocEntry(read(`docs/${doc.file}`), doc.file),
    );
  await add(
    context,
    API_REFERENCE.slug,
    apiReferenceEntry(read(API_REFERENCE.file)),
  );
};

/** Adds the page `slug` to the store. */
const add = async (context, slug, { title, description, body }) => {
  const data = await context.parseData({
    id: slug,
    data: { title, description },
  });
  context.store.set({
    id: slug,
    data,
    body,
    // Starlight places a page in the sidebar by its path in the collection;
    // the page's source is a file elsewhere, which no Starlight feature reads.
    filePath: `src/content/docs/${slug}.md`,
    rendered: await context.renderMarkdown(body),
    digest: context.generateDigest(body),
  });
};

// Starlight's docs collection, plus the developer docs rendered from their
// files in docs/ (NOC-361; which files and how: repo-docs.mjs).
import { readFileSync } from "node:fs";
import { docsLoader } from "@astrojs/starlight/loaders";
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
    const files = REPO_DOCS.map(
      (doc) => new URL(`docs/${doc.file}`, root).pathname,
    );
    context.watcher?.add(files);
    context.watcher?.on("change", (path) => {
      if (files.includes(path)) addRepoDocs(context, root);
    });
  },
});

/** Adds every rendered doc of the repository at `root` to the store. */
const addRepoDocs = async (context, root) => {
  for (const doc of REPO_DOCS) {
    const url = new URL(`docs/${doc.file}`, root);
    const source = readFileSync(url, "utf8");
    const { title, description, body } = repoDocEntry(source, doc.file);
    const data = await context.parseData({
      id: doc.slug,
      data: { title, description },
    });
    context.store.set({
      id: doc.slug,
      data,
      body,
      // Starlight places a page in the sidebar by its path in the collection;
      // the page's source is docs/<file>, which no Starlight feature reads.
      filePath: `src/content/docs/${doc.slug}.md`,
      rendered: await context.renderMarkdown(body),
      digest: context.generateDigest(body),
    });
  }
};

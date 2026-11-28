// Bundles the TipTap test page (page.mjs) with esbuild into
// target/sdk-browser/tiptap/page.js: TipTap from packages/tiptap's
// node_modules, @nocentraldb/tiptap from its source, the SDK as the page's
// /sdk/index.js and Yjs as the import map's.
import { join } from "node:path";
import { build } from "esbuild";
import { ROOT } from "../build.mjs";

export const BUNDLE = join(ROOT, "target", "sdk-browser", "tiptap", "page.js");

export const bundle = () =>
  build({
    entryPoints: [join(ROOT, "scripts", "sdk-browser", "tiptap", "page.mjs")],
    bundle: true,
    format: "esm",
    outfile: BUNDLE,
    nodePaths: [join(ROOT, "packages", "tiptap", "node_modules")],
    alias: {
      "@nocentraldb/tiptap": join(
        ROOT,
        "packages",
        "tiptap",
        "src",
        "index.ts",
      ),
    },
    plugins: [
      {
        name: "page-modules",
        setup: (b) => {
          b.onResolve({ filter: /^@nocentraldb\/sdk$/ }, () => ({
            path: "/sdk/index.js",
            external: true,
          }));
          b.onResolve({ filter: /^yjs$/ }, () => ({
            path: "yjs",
            external: true,
          }));
        },
      },
    ],
    logLevel: "warning",
  });

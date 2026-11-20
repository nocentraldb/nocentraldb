// @ts-check
import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";
import starlightLlmsTxt from "starlight-llms-txt";
import { currentInstallLines, expressiveNpmLine } from "./src/install.mjs";
import { REPO_DOCS } from "./src/repo-docs.mjs";

// The SDK's npm line in the docs follows release-notes/index.json (NOC-361).
const { npm } = currentInstallLines(new URL("../", import.meta.url));

export default defineConfig({
  site: "https://nocentraldb.io",
  output: "static",
  // The erasure page moved from /shredding/ (owner, 2026-11-20, Q5); old links still arrive.
  redirects: { "/docs/concepts/shredding/": "/docs/concepts/erasure/" },
  // Inline every stylesheet, so a built page also renders where only the
  // HTML file is served, such as a phone preview of the draft.
  build: { inlineStylesheets: "always" },
  integrations: [
    starlight({
      title: "NoCentralDB",
      logo: {
        src: "./src/assets/brand/ncdb-wordmark-dark.svg",
        replacesTitle: true,
      },
      favicon: "/brand/ncdb-favicon-dark.ico",
      components: {
        ThemeProvider: "./src/components/starlight/ThemeProvider.astro",
        ThemeSelect: "./src/components/starlight/ThemeSelect.astro",
        SiteTitle: "./src/components/starlight/SiteTitle.astro",
      },
      description:
        "A local-first database for apps where people and agents work on the same data.",
      social: [
        {
          icon: "github",
          label: "GitHub",
          href: "https://github.com/nocentraldb/nocentraldb",
        },
      ],
      customCss: ["./src/styles/tokens.css", "./src/styles/starlight.css"],
      head: [
        {
          tag: "link",
          attrs: { rel: "preconnect", href: "https://fonts.googleapis.com" },
        },
        {
          tag: "link",
          attrs: {
            rel: "preconnect",
            href: "https://fonts.gstatic.com",
            crossorigin: true,
          },
        },
      ],
      sidebar: [
        { label: "Getting started", slug: "docs" },
        { label: "Overview", slug: "docs/how-it-works" },
        // The docs rendered from docs/, the two guides first (NOC-361).
        ...REPO_DOCS.slice(0, 2).map(({ label, slug }) => ({ label, slug })),
        {
          label: "Concepts",
          items: [{ autogenerate: { directory: "docs/concepts" } }],
        },
        ...REPO_DOCS.slice(2).map(({ label, slug }) => ({ label, slug })),
        { label: "API", slug: "docs/api" },
        { label: "RFCs", slug: "docs/rfcs" },
      ],
      // A shell command sits in a plain frame: the terminal frame's empty
      // title bar took a third of a one-line command's height (D1, NOC-360).
      expressiveCode: {
        defaultProps: { frame: "code" },
        plugins: [expressiveNpmLine({ npm })],
      },
      plugins: [starlightLlmsTxt()],
    }),
  ],
});

// @ts-check
import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";
import starlightLlmsTxt from "starlight-llms-txt";

export default defineConfig({
  site: "https://nocentraldb.io",
  output: "static",
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
        {
          label: "Concepts",
          items: [{ autogenerate: { directory: "docs/concepts" } }],
        },
        { label: "API", slug: "docs/api" },
        { label: "RFCs", slug: "docs/rfcs" },
      ],
      plugins: [starlightLlmsTxt()],
    }),
  ],
});

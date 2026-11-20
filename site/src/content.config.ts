import { defineCollection } from "astro:content";
import { docsSchema } from "@astrojs/starlight/schema";
import { docsWithRepoLoader } from "./docs-loader.mjs";

export const collections = {
  docs: defineCollection({
    loader: docsWithRepoLoader(),
    schema: docsSchema(),
  }),
};

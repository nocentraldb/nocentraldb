// A Markdown copy of every docs page at its path plus `.md` (docs/api.md for
// /docs/api/), next to the llms.txt files of starlight-llms-txt, which does
// not serve one per page (NOC-211).

import { getCollection } from "astro:content";
import { pathToFileURL } from "node:url";
import type { APIRoute, GetStaticPaths } from "astro";
import { currentInstallLines, withNpmLine } from "../install.mjs";

// The SDK's npm line follows release-notes/index.json, as on the page
// (NOC-361). The site builds in site/, as CodeSection.astro reads it.
const { npm } = currentInstallLines(pathToFileURL(`${process.cwd()}/../`));

export const getStaticPaths: GetStaticPaths = async () => {
  const entries = await getCollection("docs");
  return entries.map((entry) => ({
    params: { slug: entry.id },
    props: { entry },
  }));
};

export const GET: APIRoute = ({ props }) => {
  const { data, body } = props.entry;
  const text = `# ${data.title}\n\n${data.description ? `> ${data.description}\n\n` : ""}${withNpmLine(body ?? "", npm)}`;
  return new Response(text, {
    headers: { "Content-Type": "text/markdown; charset=utf-8" },
  });
};

// The developer docs that the site renders from their files in docs/ (NOC-361,
// the owner's direction of 2026-11-19): one source, which the repository's
// checks keep compiling, and no copy in site/. docs-loader.mjs adds them to
// Starlight's docs collection; this module turns one file into a page.
//
// Which files: those written for app developers. Contributor and operator
// docs (release management, the relay's deploy, the repository, the AWS
// Emulator, the browser storage checklist) stay in the repository.
//
// A link in a rendered doc leads to the site when it points to another
// rendered doc, and to GitHub for any other file of the repository. Its npm
// install line follows release-notes/index.json, like every docs page's
// (install.mjs).
import { existsSync, statSync } from "node:fs";
import { posix } from "node:path";
import { fileURLToPath } from "node:url";
import { fromMarkdown } from "mdast-util-from-markdown";
import { gfmFromMarkdown } from "mdast-util-gfm";
import { gfm } from "micromark-extension-gfm";

const GIT = "https://github.com/nocentraldb/nocentraldb";
const ROOT = fileURLToPath(new URL("../../", import.meta.url));

/** The rendered docs, in their sidebar order after "Overview". */
export const REPO_DOCS = [
  {
    file: "getting-started.md",
    slug: "docs/guides/rust",
    label: "Getting started in Rust",
  },
  {
    file: "getting-started-browser.md",
    slug: "docs/guides/browser",
    label: "Getting started in the browser",
  },
  {
    file: "bulk-import.md",
    slug: "docs/guides/bulk-import",
    label: "Bursts of writes",
  },
  {
    file: "spaces-and-invitations.md",
    slug: "docs/guides/spaces-and-invitations",
    label: "Spaces and invitations",
  },
  {
    file: "browser-durability.md",
    slug: "docs/browser-durability",
    label: "Browser apps and durability",
  },
  {
    file: "browser-storage.md",
    slug: "docs/browser-storage",
    label: "How long a browser keeps a copy",
  },
  { file: "use-cases.md", slug: "docs/use-cases", label: "Use cases" },
  { file: "faq.md", slug: "docs/faq", label: "FAQ" },
  { file: "glossary.md", slug: "docs/glossary", label: "Glossary" },
];

const SLUGS = new Map(REPO_DOCS.map((doc) => [`docs/${doc.file}`, doc.slug]));

/** The plain text of a syntax tree node. */
const text = (node) =>
  "value" in node ? node.value : (node.children ?? []).map(text).join("");

/** Where the link `url`, written in docs/`file`, leads on the site. */
const siteUrl = (url, file) => {
  if (
    /^[a-z][a-z0-9+.-]*:/i.test(url) ||
    url.startsWith("#") ||
    url.startsWith("/")
  )
    return url;
  const hash = url.indexOf("#");
  const path = hash < 0 ? url : url.slice(0, hash);
  const anchor = hash < 0 ? "" : url.slice(hash);
  const target = posix.normalize(posix.join("docs", posix.dirname(file), path));
  if (target.startsWith("..") || posix.isAbsolute(target))
    throw new Error(`docs/${file} links outside the repository: ${url}`);
  const slug = SLUGS.get(target);
  if (slug) return `/${slug}/${anchor}`;
  const local = posix.join(ROOT, target);
  const dir =
    path.endsWith("/") || (existsSync(local) && statSync(local).isDirectory());
  const clean = target.replace(/\/$/, "");
  return `${GIT}/${dir ? "tree" : "blob"}/main/${clean}${dir ? "/" : ""}${anchor}`;
};

/**
 * The page for docs/`file`, whose text is `source`: its title (the first
 * heading, which leaves the body), its description (the first sentence of the
 * first paragraph) and its body, with its links rewritten.
 */
export const repoDocEntry = (source, file) => {
  const tree = fromMarkdown(source, {
    extensions: [gfm()],
    mdastExtensions: [gfmFromMarkdown()],
  });
  const edits = [];
  const heading = tree.children.find(
    (n) => n.type === "heading" && n.depth === 1,
  );
  if (!heading) throw new Error(`docs/${file} has no first heading`);
  const title = text(heading);
  let end = heading.position.end.offset;
  while (source[end] === "\n") end += 1;
  edits.push([heading.position.start.offset, end, ""]);
  const first = tree.children.find(
    (n) =>
      n.type === "paragraph" &&
      n.position.start.offset > heading.position.end.offset,
  );
  const paragraph = first ? text(first).replace(/\s+/g, " ").trim() : "";
  const description = paragraph.match(/^.*?[.!?](?=\s|$)/)?.[0] ?? paragraph;

  const visit = (node) => {
    if (["link", "image", "definition"].includes(node.type)) {
      const url = siteUrl(node.url, file);
      if (url !== node.url) {
        const { start, end } = node.position;
        const at = source.lastIndexOf(node.url, end.offset);
        if (at < start.offset)
          throw new Error(`docs/${file}: cannot place the link ${node.url}`);
        edits.push([at, at + node.url.length, url]);
      }
    }
    for (const child of node.children ?? []) visit(child);
  };
  visit(tree);

  let body = source;
  for (const [from, to, value] of edits.sort((a, b) => b[0] - a[0]))
    body = body.slice(0, from) + value + body.slice(to);
  return { title, description, body };
};

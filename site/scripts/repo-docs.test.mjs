// The site renders the developer docs from their files in docs/ (NOC-361, the
// owner's direction of 2026-11-19): one source, no copies in site/. These
// tests check how a file of docs/ becomes a docs page: its title, its
// description, and its links, which lead to the site for a rendered doc and
// to GitHub for every other file of the repository.
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { REPO_DOCS, repoDocEntry } from "../src/repo-docs.mjs";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const GIT = "https://github.com/nocentraldb/nocentraldb";
const npm = "npm install @nocentraldb/sdk@beta";
const entry = (source, file = "getting-started-browser.md") =>
  repoDocEntry(source, file, { npm });

test("every rendered doc exists in docs/, once, with its own page", () => {
  assert.ok(REPO_DOCS.length > 0);
  for (const doc of REPO_DOCS)
    assert.ok(existsSync(join(repo, "docs", doc.file)), `docs/${doc.file}`);
  const files = REPO_DOCS.map((doc) => doc.file);
  const slugs = REPO_DOCS.map((doc) => doc.slug);
  assert.equal(new Set(files).size, files.length);
  assert.equal(new Set(slugs).size, slugs.length);
  for (const slug of slugs) assert.match(slug, /^docs\/[a-z0-9/-]+$/);
});

test("the two guides, browser durability and the use cases are rendered", () => {
  const files = REPO_DOCS.map((doc) => doc.file);
  for (const file of [
    "getting-started.md",
    "getting-started-browser.md",
    "browser-durability.md",
    "use-cases.md",
  ])
    assert.ok(files.includes(file), file);
});

test("contributor and operator docs stay in the repository", () => {
  const files = REPO_DOCS.map((doc) => doc.file);
  for (const file of [
    "release-management.md",
    "relay-deploy.md",
    "repository.md",
    "aws-emulator.md",
    "browser-storage-checklist.md",
  ])
    assert.ok(!files.includes(file), file);
});

test("the first heading is the page's title and leaves the body", () => {
  const page = entry("# A guide\n\nThe first sentence. The second.\n\n## Next\n");
  assert.equal(page.title, "A guide");
  assert.ok(!page.body.includes("# A guide"));
  assert.ok(page.body.startsWith("The first sentence."));
});

test("the description is the first paragraph's first sentence, as plain text", () => {
  const page = entry(
    "# T\n\nThis guide builds a task list on the package `@nocentraldb/sdk`, in a [web page](x.md). Then more.\n",
  );
  assert.equal(
    page.description,
    "This guide builds a task list on the package @nocentraldb/sdk, in a web page.",
  );
});

test("a link to a rendered doc becomes its site link, its anchor kept", () => {
  const page = entry(
    "# T\n\nSee [durability](browser-durability.md#what-is-lost) and [the guide](./getting-started.md).\n",
  );
  assert.match(page.body, /\(\/docs\/browser-durability\/#what-is-lost\)/);
  assert.match(page.body, /\(\/docs\/guides\/rust\/\)/);
});

test("a link to another file of the repository becomes its GitHub link", () => {
  const page = entry(
    "# T\n\n[RFC](rfc/0006-local-view.md#durability) [app](../apps/tasks-ts/README.md) [deploy](relay-deploy.md) [apps](../apps/tasks-ts/)\n",
  );
  assert.ok(
    page.body.includes(`(${GIT}/blob/main/docs/rfc/0006-local-view.md#durability)`),
  );
  assert.ok(page.body.includes(`(${GIT}/blob/main/apps/tasks-ts/README.md)`));
  assert.ok(page.body.includes(`(${GIT}/blob/main/docs/relay-deploy.md)`));
  assert.ok(page.body.includes(`(${GIT}/tree/main/apps/tasks-ts/)`));
});

test("a reference definition is rewritten like an inline link", () => {
  const page = entry("# T\n\nSee [the RFC][rfc].\n\n[rfc]: rfc/0009-sdk-api.md\n");
  assert.ok(page.body.includes(`[rfc]: ${GIT}/blob/main/docs/rfc/0009-sdk-api.md`));
});

test("absolute links, anchors on the page and links in code stay as they are", () => {
  const source =
    "# T\n\n[x](https://example.com/a.md) [y](#below) `[z](rfc/a.md)`\n\n```md\n[w](rfc/b.md)\n```\n";
  const page = entry(source);
  assert.ok(page.body.includes("(https://example.com/a.md)"));
  assert.ok(page.body.includes("(#below)"));
  assert.ok(page.body.includes("`[z](rfc/a.md)`"));
  assert.ok(page.body.includes("[w](rfc/b.md)"));
});

test("a link that leaves the repository is refused", () => {
  assert.throws(() => entry("# T\n\n[x](../../outside.md)\n"), /outside/);
});

test("the npm install line follows the current pre-release", () => {
  const page = entry(
    "# T\n\nRun `npm install @nocentraldb/sdk@alpha`, or:\n\n```sh\nnpm install @nocentraldb/sdk@alpha\n```\n",
  );
  assert.ok(!page.body.includes("@nocentraldb/sdk@alpha"));
  assert.equal(page.body.split(npm).length - 1, 2);
});

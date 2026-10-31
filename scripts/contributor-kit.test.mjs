// Asserts the contributor kit (mt08.yc1s): the issue forms, the RFC feedback
// discussion form, the issue chooser config and the pull request template have
// the shape GitHub accepts and the fields and labels the kit promises; every
// kit file lies on the fragment allowlist, so the kit itself never needs a
// change fragment; the fragment example in CONTRIBUTING.md passes the
// fragment schema; and every relative link in the kit's Markdown resolves to a
// file, and to a heading when it names one.
//
// GitHub validates issue forms only when it renders them. These checks catch
// the usual mistakes before that: a missing top-level key, an unknown element
// type, a duplicate or malformed id, a dropdown without options, a label typo.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";
import { ALLOWLIST } from "./release/check-fragments.mjs";
import { validateFragment } from "./release/lib/fragments.mjs";
import { matchesAny } from "./release/lib/glob.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");
const yaml = (path) => parse(read(path));

const ISSUE_FORMS = [
  ".github/ISSUE_TEMPLATE/bug_report.yml",
  ".github/ISSUE_TEMPLATE/milestone_offer.yml",
];
const DISCUSSION_FORM = ".github/DISCUSSION_TEMPLATE/rfc-feedback.yml";
const CHOOSER = ".github/ISSUE_TEMPLATE/config.yml";
const PR_TEMPLATE = ".github/PULL_REQUEST_TEMPLATE.md";
const MARKDOWN = [
  "CONTRIBUTING.md",
  "README.md",
  ".github/AI_POLICY.md",
  ".github/SECURITY.md",
  ".github/CODE_OF_CONDUCT.md",
  PR_TEMPLATE,
];
const KIT = [...MARKDOWN, ...ISSUE_FORMS, DISCUSSION_FORM, CHOOSER];

const ELEMENT_TYPES = [
  "markdown",
  "textarea",
  "input",
  "dropdown",
  "checkboxes",
];
const ID = /^[A-Za-z0-9_-]+$/;

/** The body of a form, checked against GitHub's form schema. */
function checkBody(path, body) {
  assert.ok(Array.isArray(body) && body.length > 0, `${path}: empty body`);
  const ids = new Set();
  for (const element of body) {
    assert.ok(
      ELEMENT_TYPES.includes(element.type),
      `${path}: unknown element type ${element.type}`,
    );
    if (element.type === "markdown") {
      assert.equal(typeof element.attributes?.value, "string", path);
      continue;
    }
    assert.match(element.id ?? "", ID, `${path}: element without a valid id`);
    assert.ok(!ids.has(element.id), `${path}: duplicate id ${element.id}`);
    ids.add(element.id);
    assert.equal(typeof element.attributes?.label, "string", element.id);
    if (element.type === "dropdown" || element.type === "checkboxes") {
      const options = element.attributes.options;
      assert.ok(Array.isArray(options) && options.length > 0, element.id);
    }
  }
}

/** The non-markdown elements of a form, by id. */
const fields = (form) =>
  Object.fromEntries(
    form.body.filter((e) => e.type !== "markdown").map((e) => [e.id, e]),
  );
const required = (element) => element.validations?.required === true;

test("every kit file exists and lies on the fragment allowlist", () => {
  for (const path of KIT) {
    assert.ok(existsSync(join(root, path)), `${path} is missing`);
    assert.ok(matchesAny(path, ALLOWLIST), `${path} would need a fragment`);
  }
});

test("the issue forms follow GitHub's issue form schema", () => {
  for (const path of ISSUE_FORMS) {
    const form = yaml(path);
    assert.equal(typeof form.name, "string", `${path}: name`);
    assert.equal(typeof form.description, "string", `${path}: description`);
    checkBody(path, form.body);
  }
});

test("the bug report requires what happened, a reproduction, version, OS and component", () => {
  const form = yaml(".github/ISSUE_TEMPLATE/bug_report.yml");
  assert.deepEqual(form.labels, ["needs triage"]);
  const f = fields(form);
  for (const id of [
    "what-happened",
    "reproduction",
    "version",
    "os",
    "component",
  ]) {
    assert.ok(f[id], `bug_report: no field ${id}`);
    assert.ok(required(f[id]), `bug_report: ${id} is not required`);
  }
  assert.deepEqual(f.component.attributes.options, [
    "core",
    "Rust SDK",
    "TS SDK",
    "CLI",
    "relay",
    "docs",
  ]);
  assert.ok(f["rfc-promise"], "bug_report: no RFC promise field");
  assert.ok(!required(f["rfc-promise"]), "the RFC promise is optional");
  assert.equal(f["help-fix"]?.type, "checkboxes");
  assert.ok(!required(f["help-fix"].attributes.options[0]));
  const asked = JSON.stringify(form).toLowerCase();
  assert.ok(!/full name|affiliation/.test(asked), "never ask for identity");
});

test("the milestone offer asks for milestone, experience, approach and time frame", () => {
  const form = yaml(".github/ISSUE_TEMPLATE/milestone_offer.yml");
  assert.deepEqual(form.labels, ["milestone-offer"]);
  const f = fields(form);
  for (const id of ["milestone", "experience", "approach", "timeframe"]) {
    assert.ok(f[id], `milestone_offer: no field ${id}`);
  }
});

test("the RFC feedback discussion form follows the discussion form schema", () => {
  const form = yaml(DISCUSSION_FORM);
  for (const key of Object.keys(form)) {
    assert.ok(["title", "labels", "body"].includes(key), `unknown key ${key}`);
  }
  checkBody(DISCUSSION_FORM, form.body);
  const f = fields(form);
  for (const id of ["rfc", "feedback", "situation", "breaks"]) {
    assert.ok(f[id], `rfc-feedback: no field ${id}`);
  }
  assert.ok(required(f.rfc) && required(f.feedback));
});

test("the issue chooser disables blank issues and links security, discussions and the RFC", () => {
  const config = yaml(CHOOSER);
  assert.equal(config.blank_issues_enabled, false);
  const links = config.contact_links;
  for (const link of links) {
    for (const key of ["name", "url", "about"]) {
      assert.equal(typeof link[key], "string", `contact link ${key}`);
    }
  }
  const urls = links.map((l) => l.url);
  const repo = "https://github.com/nocentraldb/nocentraldb";
  assert.ok(urls.includes(`${repo}/security/advisories/new`));
  assert.ok(urls.includes(`${repo}/discussions`));
  assert.ok(
    urls.includes(
      `${repo}/blob/main/docs/rfc/0001-nocentraldb.md#how-to-give-advice`,
    ),
  );
});

test("the pull request template has its sections, in order", () => {
  const headings = read(PR_TEMPLATE)
    .split("\n")
    .filter((line) => line.startsWith("## "));
  assert.deepEqual(headings, [
    "## Summary",
    "## How it was tested",
    "## Change fragment",
    "## Security considerations",
    "## Authorship",
  ]);
  const text = read(PR_TEMPLATE);
  for (const choice of [
    "Human",
    "Agent under close human direction",
    "Agent with human review",
  ]) {
    assert.ok(text.includes(choice), `authorship choice ${choice}`);
  }
  assert.ok(!/\bnobody\b/i.test(text), "no 'nobody answers review' option");
  assert.match(text, /Follow-up:/);
});

/** GitHub's heading anchor: lower case, punctuation dropped, spaces to dashes. */
const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .replace(/\s/g, "-");
const anchors = (markdown) =>
  new Set(
    markdown
      .split("\n")
      .filter((line) => /^#{1,6} /.test(line))
      .map((line) => slug(line.replace(/^#+ /, ""))),
  );

test("every relative link in the kit resolves, anchors included", () => {
  for (const path of MARKDOWN) {
    const text = read(path).replace(/```[\s\S]*?```/g, "");
    for (const [, target] of text.matchAll(/\]\(([^)\s]+)\)/g)) {
      if (/^[a-z]+:/.test(target)) continue;
      const [file, anchor] = target.split("#");
      const resolved = file ? normalize(join(dirname(path), file)) : path;
      assert.ok(existsSync(join(root, resolved)), `${path}: ${target}`);
      if (anchor && resolved.endsWith(".md")) {
        assert.ok(
          anchors(read(resolved)).has(anchor),
          `${path}: no heading #${anchor} in ${resolved}`,
        );
      }
    }
  }
});

test("the fragment example in CONTRIBUTING.md passes the fragment schema", () => {
  const example = /```md\n([\s\S]*?)```/.exec(read("CONTRIBUTING.md"));
  assert.ok(example, "CONTRIBUTING.md shows no fragment example");
  assert.deepEqual(validateFragment("example", example[1]), []);
});

test("the README has a Contributing section that links the kit", () => {
  const readme = read("README.md");
  const section = /\n## Contributing\n([\s\S]*?)(?=\n## |$)/.exec(readme);
  assert.ok(section, "README.md has no Contributing section");
  for (const target of [
    "CONTRIBUTING.md",
    ".github/AI_POLICY.md",
    ".github/SECURITY.md",
    ".github/CODE_OF_CONDUCT.md",
  ]) {
    assert.ok(section[1].includes(`](${target})`), `README links ${target}`);
  }
});

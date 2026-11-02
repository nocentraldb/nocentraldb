import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  COMPONENTS,
  demoteSections,
  fragmentsAt,
  fragmentsIn,
  listFragments,
  parseFragment,
  RELEASED,
  readFragments,
  SECTIONS,
  SLUG,
  splitSections,
  TICKET,
  TYPES,
  validateFragment,
} from "./fragments.mjs";
import { GIT_ENV, makeRepo } from "./test-repo.mjs";

const feature = `---
type: feature
tickets: [mt08.1qaz]
components: [core]
---
## Problem
Users could not do X.

## Use
Call \`y()\`.
`;

test("parses front matter and level-2 sections", () => {
  const { front, sections } = parseFragment(feature);
  assert.deepEqual(front, {
    type: "feature",
    tickets: ["mt08.1qaz"],
    components: ["core"],
  });
  assert.deepEqual(sections, ["Problem", "Use"]);
});

test("a valid feature has no violations", () => {
  assert.deepEqual(validateFragment("add-y", feature), []);
});

const cases = {
  "no front matter": ["add-y", "## Problem\n", /no front matter/],
  "bad slug": ["Add_Y", feature, /slug/],
  "unknown type": [
    "add-y",
    feature.replace("feature", "chore"),
    /type must be one of/,
  ],
  "empty tickets": ["add-y", feature.replace("[mt08.1qaz]", "[]"), /tickets/],
  "bad ticket id": ["add-y", feature.replace("mt08.1qaz", "MT08-1"), /tickets/],
  "empty components": ["add-y", feature.replace("[core]", "[]"), /components/],
  "unknown component": [
    "add-y",
    feature.replace("[core]", "[web]"),
    /components/,
  ],
  "missing section": [
    "add-y",
    feature.replace("## Use\nCall `y()`.\n", ""),
    /missing section ## Use/,
  ],
  "unexpected section": [
    "add-y",
    `${feature}\n## Notes\nx\n`,
    /unexpected section ## Notes/,
  ],
  "introduced on a feature": [
    "add-y",
    feature.replace("components:", "introduced: cycle\ncomponents:"),
    /introduced is only allowed on fix/,
  ],
  "fix without introduced": [
    "fix-z",
    feature
      .replace("feature", "fix")
      .replace(
        /## Problem[\s\S]*/,
        "## Symptom\na\n## When\nb\n## Cause\nc\n## Fix\nd\n",
      ),
    /fix needs introduced/,
  ],
  "fix with a bad introduced": [
    "fix-z",
    feature
      .replace("feature", "fix")
      .replace("components:", "introduced: v0.1\ncomponents:")
      .replace(
        /## Problem[\s\S]*/,
        "## Symptom\na\n## When\nb\n## Cause\nc\n## Fix\nd\n",
      ),
    /fix needs introduced/,
  ],
  "unknown field": [
    "add-y",
    feature.replace("components:", "author: me\ncomponents:"),
    /unknown field author/,
  ],
  "internal with a section": [
    "bump",
    "---\ntype: internal\ntickets: [mt08.1qaz]\ncomponents: [core]\n---\n## Problem\nx\n",
    /internal has a one-line body/,
  ],
  "internal with two lines": [
    "bump",
    "---\ntype: internal\ntickets: [mt08.1qaz]\ncomponents: [core]\n---\nline one\nline two\n",
    /internal has a one-line body/,
  ],
  "empty body": [
    "add-y",
    feature.replace(/## Problem[\s\S]*/, ""),
    /missing section ## Problem/,
  ],
};
for (const [name, [slug, text, rule]] of Object.entries(cases)) {
  test(`rejects: ${name}`, () => {
    const errors = validateFragment(slug, text);
    assert.ok(
      errors.some((e) => rule.test(e)),
      `${name}: ${JSON.stringify(errors)}`,
    );
  });
}

test("accepts every type with its sections", () => {
  const ok = {
    breaking: "## Problem\na\n## Use\nb\n## Migration\nc\n",
    fix: "## Symptom\na\n## When\nb\n## Cause\nc\n## Fix\nd\n",
    deprecation: "## Deprecated\na\n## Replacement\nb\n## Removed in\n0.3.0\n",
    security: "## Description\na\n## Advisory\npending\n",
    internal: "Bumped the build.\n",
  };
  for (const [type, body] of Object.entries(ok)) {
    const introduced = type === "fix" ? "introduced: 0.1.0\n" : "";
    const text = `---\ntype: ${type}\ntickets: [mt08.1qaz]\n${introduced}components: [sdk-ts, cli]\n---\n${body}`;
    assert.deepEqual(validateFragment(`x-${type}`, text), [], type);
  }
  const cycle =
    "---\ntype: fix\ntickets: [mt08.1qaz]\nintroduced: cycle\ncomponents: [core]\n---\n## Symptom\na\n## When\nb\n## Cause\nc\n## Fix\nd\n";
  assert.deepEqual(validateFragment("fix-cycle", cycle), []);
});

test("a level-2 heading inside a fenced code block is not a section", () => {
  const withCode = feature.replace(
    "Call `y()`.\n",
    "Call it:\n\n```sh\n## not a heading\nncdb y\n```\n\n~~~md\n## Nor this\n~~~\n",
  );
  assert.deepEqual(parseFragment(withCode).sections, ["Problem", "Use"]);
  assert.deepEqual(validateFragment("add-y", withCode), []);
});

test("rejects front matter that is not a mapping or not YAML", () => {
  assert.ok(
    validateFragment("add-y", "---\n- a\n- b\n---\n").some((e) =>
      /not a mapping/.test(e),
    ),
  );
  assert.ok(validateFragment("add-y", "---\ntype: [feature\n---\n").length > 0);
  assert.throws(() => parseFragment("---\n- a\n---\n"), /not a mapping/);
  assert.throws(() => parseFragment("## Problem\n"), /no front matter/);
});

const tree = (files) => {
  const dir = mkdtempSync(join(tmpdir(), "fragments-"));
  for (const [name, text] of Object.entries(files))
    writeFileSync(join(dir, name), text);
  return dir;
};

test("listFragments lists every *.md except README.md, sorted by slug", (t) => {
  const dir = tree({
    "b-two.md": feature,
    "a-one.md": feature,
    "README.md": "# guide\n",
    "notes.txt": "x",
  });
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  assert.deepEqual(listFragments(dir), [
    { slug: "a-one", file: join(dir, "a-one.md") },
    { slug: "b-two", file: join(dir, "b-two.md") },
  ]);
});

test("readFragments returns the parsed fragments", (t) => {
  const dir = tree({ "add-y.md": feature, "README.md": "# guide\n" });
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const [one, ...rest] = readFragments(dir);
  assert.equal(rest.length, 0);
  assert.equal(one.slug, "add-y");
  assert.equal(one.file, join(dir, "add-y.md"));
  assert.equal(one.front.type, "feature");
  assert.deepEqual(one.sections, ["Problem", "Use"]);
  assert.match(one.body, /Users could not do X/);
});

test("readFragments throws on the first invalid fragment, naming its file", (t) => {
  const dir = tree({
    "add-y.md": feature,
    "bad.md": feature.replace("[core]", "[web]"),
  });
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  assert.throws(
    () => readFragments(dir),
    (error) => error.message.startsWith(`${join(dir, "bad.md")}: components`),
  );
});

test("the exported vocabularies match docs/release-management.md", () => {
  assert.deepEqual(TYPES, [
    "breaking",
    "feature",
    "fix",
    "deprecation",
    "security",
    "internal",
  ]);
  assert.deepEqual(COMPONENTS, ["core", "sdk-rust", "sdk-ts", "cli", "relay"]);
  assert.deepEqual(Object.keys(SECTIONS), TYPES);
  for (const slug of ["a", "0", "add-y", "a1-b2"])
    assert.ok(SLUG.test(slug), slug);
  for (const slug of ["-a", "A", "a_b", "a.b", "a/b", "", "a b"])
    assert.ok(!SLUG.test(slug), slug);
  assert.ok(
    TICKET.test("mt08.1qaz") &&
      !TICKET.test("mt08") &&
      !TICKET.test("MT08.1qaz") &&
      !TICKET.test("mt08.1qaz.x"),
  );
  assert.ok(
    RELEASED.test("0.1.0") &&
      !RELEASED.test("v0.1.0") &&
      !RELEASED.test("0.1.0-beta.1") &&
      !RELEASED.test("0.1"),
  );
});

test("fragmentsIn picks the fragments among repository paths, as listFragments does", () => {
  assert.deepEqual(
    fragmentsIn([
      "changes/z-y.md",
      "changes/README.md",
      "changes/a-x.md",
      "changes/sub/deep.md",
      "changes/notes.txt",
      "other/b.md",
      "changesx/c.md",
    ]),
    [
      { slug: "a-x", file: "changes/a-x.md" },
      { slug: "z-y", file: "changes/z-y.md" },
    ],
  );
});

test("fragmentsAt reads and validates every fragment as it stands at a commit", (t) => {
  const repo = makeRepo();
  t.after(() => repo.dispose());
  const first = repo.commit("one", {
    "changes/README.md": "# guide\n",
    "changes/add-y.md": feature,
  });
  repo.commit("two", { "changes/bad.md": "no front matter\n" });
  const opts = { cwd: repo.dir, env: GIT_ENV };
  const [valid] = fragmentsAt(first, opts);
  assert.equal(fragmentsAt(first, opts).length, 1);
  assert.equal(valid.slug, "add-y");
  assert.equal(valid.file, "changes/add-y.md");
  assert.deepEqual(valid.errors, []);
  assert.equal(valid.front.type, "feature");
  assert.deepEqual(valid.sections, ["Problem", "Use"]);
  const [, bad] = fragmentsAt("HEAD", opts);
  assert.equal(bad.file, "changes/bad.md");
  assert.match(bad.errors[0], /no front matter/);
  assert.equal(bad.front, undefined);
});

test("demoteSections turns ## headings into ### outside fenced code blocks only", () => {
  const body =
    "## Problem\nText ## not a heading\n### Deeper\n```md\n## inside\n```\n~~~\n## also inside\n~~~\n## Use\n";
  assert.equal(
    demoteSections(body),
    "### Problem\nText ## not a heading\n### Deeper\n```md\n## inside\n```\n~~~\n## also inside\n~~~\n### Use\n",
  );
});

test("demoteSections demotes sections by the given number of levels, fences untouched", () => {
  const body = "## Problem\n### Deeper\n```md\n## inside\n```\n## Use\n";
  assert.equal(
    demoteSections(body, 2),
    "#### Problem\n### Deeper\n```md\n## inside\n```\n#### Use\n",
  );
});

test("splitSections splits a body at its ## headings outside fences, text trimmed, the preamble kept", () => {
  const body =
    "Lead.\n\n## Problem\n\nNo x.\n\n## Use\n\n```md\n## not a section\n```\n\n## Empty\n";
  assert.deepEqual(splitSections(body), {
    preamble: "Lead.",
    sections: [
      { name: "Problem", text: "No x." },
      { name: "Use", text: "```md\n## not a section\n```" },
      { name: "Empty", text: "" },
    ],
  });
});

const withIssues = (value) =>
  feature.replace("components:", `issues: ${value}\ncomponents:`);

test("issues is optional; a list of positive issue numbers is valid", () => {
  assert.deepEqual(validateFragment("add-y", withIssues("[42, 57]")), []);
  assert.deepEqual(validateFragment("add-y", withIssues("[1]")), []);
  assert.deepEqual(
    parseFragment(withIssues("[42, 57]")).front.issues,
    [42, 57],
  );
});

test("issues: a bad value is rejected and named", () => {
  const rejected = {
    "an empty list": ["[]", /^issues must be a non-empty list/],
    "a number, not a list": ["42", /^issues must be a non-empty list.*got 42$/],
    "no value": ["", /^issues must be a non-empty list.*got null$/],
    zero: ["[0]", /^issues: 0 is not a positive integer$/],
    "a negative number": ["[42, -1]", /^issues: -1 is not a positive integer$/],
    "a fraction": ["[4.5]", /^issues: 4\.5 is not a positive integer$/],
    "a string": ['["57"]', /^issues: "57" is not a positive integer$/],
    "a reference": ['["#57"]', /^issues: "#57" is not a positive integer$/],
    "a duplicate": ["[42, 57, 42]", /^issues: 42 is listed twice$/],
  };
  for (const [name, [value, rule]] of Object.entries(rejected)) {
    const errors = validateFragment("add-y", withIssues(value));
    assert.ok(
      errors.some((e) => rule.test(e)),
      `${name}: ${JSON.stringify(errors)}`,
    );
  }
});

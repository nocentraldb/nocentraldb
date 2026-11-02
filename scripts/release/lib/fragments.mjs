// Change fragments (changes/<slug>.md): YAML front matter between `---`
// lines, then a Markdown body whose level-2 headings are the sections.
// The rules: docs/release-management.md, "Writing a change fragment".
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { parse } from "yaml";
import { fileAt, filesAt } from "./git.mjs";

export const TYPES = [
  "breaking",
  "feature",
  "fix",
  "deprecation",
  "security",
  "internal",
];
export const COMPONENTS = ["core", "sdk-rust", "sdk-ts", "cli", "relay"];
export const SLUG = /^[a-z0-9][a-z0-9-]*$/;
export const TICKET = /^[a-z0-9]+\.[a-z0-9]+$/;
export const RELEASED = /^\d+\.\d+\.\d+$/;
export const SECTIONS = {
  breaking: ["Problem", "Use", "Migration"],
  feature: ["Problem", "Use"],
  fix: ["Symptom", "When", "Cause", "Fix"],
  deprecation: ["Deprecated", "Replacement", "Removed in"],
  security: ["Description", "Advisory"],
  internal: [],
};
const FIELDS = new Set([
  "type",
  "tickets",
  "issues",
  "introduced",
  "components",
]);

// The front matter: `---` as the first line, closed by the first line that is
// exactly `---`, followed by a newline or the end of the file.
const FRONT_MATTER = /^---\n([\s\S]*?)\n---(?:\n([\s\S]*))?$/;

export function parseFragment(text) {
  if (text.includes("\r\n"))
    throw new Error("has CRLF line endings; save it with LF line endings");
  const match = FRONT_MATTER.exec(text);
  if (!match) throw new Error("no front matter block (--- … ---) at the top");
  const front = parse(match[1]);
  if (front === null || typeof front !== "object" || Array.isArray(front)) {
    throw new Error("front matter is not a mapping");
  }
  const body = match[2] ?? "";
  return { front, body, sections: headings(body) };
}

// The lines of a Markdown body, each with whether it is prose: outside a
// fenced code block (``` or ~~~) and not a fence line itself. A `## ` line in
// an example is not prose, so it is never taken for a section.
function* classifyLines(body) {
  let fence = null;
  for (const line of body.split("\n")) {
    const marker = /^ {0,3}(`{3,}|~{3,})/.exec(line)?.[1];
    if (fence === null && marker) {
      fence = marker;
      yield { line, prose: false };
    } else if (
      fence !== null &&
      marker?.[0] === fence[0] &&
      marker.length >= fence.length &&
      line.trim() === marker
    ) {
      fence = null;
      yield { line, prose: false };
    } else yield { line, prose: fence === null };
  }
}

const SECTION = /^## (.+?)\s*$/;

/** The level-2 headings of a Markdown body, outside fenced code blocks. */
function headings(body) {
  const found = [];
  for (const { line, prose } of classifyLines(body)) {
    const heading = prose && SECTION.exec(line);
    if (heading) found.push(heading[1]);
  }
  return found;
}

/**
 * The body with its sections (`## ` headings outside fences) demoted by
 * `levels`: to `### ` by default.
 */
export function demoteSections(body, levels = 1) {
  const prefix = "#".repeat(levels);
  return Array.from(classifyLines(body), ({ line, prose }) =>
    prose && SECTION.test(line) ? `${prefix}${line}` : line,
  ).join("\n");
}

/**
 * A body split at its sections (`## ` headings outside fences): the text
 * before the first one, and each section's name and text, both trimmed.
 */
export function splitSections(body) {
  let current = { lines: [] };
  const parts = [current];
  for (const { line, prose } of classifyLines(body)) {
    const heading = prose && SECTION.exec(line);
    if (heading) {
      current = { name: heading[1], lines: [] };
      parts.push(current);
    } else current.lines.push(line);
  }
  const text = ({ lines }) => lines.join("\n").trim();
  return {
    preamble: text(parts[0]),
    sections: parts.slice(1).map((p) => ({ name: p.name, text: text(p) })),
  };
}

const isList = (value, ok) =>
  Array.isArray(value) &&
  value.length > 0 &&
  value.every((v) => typeof v === "string" && ok(v));

/** The rules for `issues`, optional: GitHub issue numbers of this repository. */
const issueErrors = (issues) => {
  if (!Array.isArray(issues) || issues.length === 0)
    return [
      `issues must be a non-empty list of GitHub issue numbers, like [42, 57]; got ${JSON.stringify(issues)}`,
    ];
  const errors = [];
  const seen = new Set();
  for (const issue of issues) {
    if (!Number.isInteger(issue) || issue <= 0)
      errors.push(`issues: ${JSON.stringify(issue)} is not a positive integer`);
    else if (seen.has(issue)) errors.push(`issues: ${issue} is listed twice`);
    seen.add(issue);
  }
  return errors;
};

export function validateFragment(slug, text) {
  const errors = [];
  if (!SLUG.test(slug))
    errors.push(`slug ${JSON.stringify(slug)} must match ${SLUG}`);
  let parsed;
  try {
    parsed = parseFragment(text);
  } catch (error) {
    return [...errors, error.message];
  }
  const { front, body, sections } = parsed;
  for (const key of Object.keys(front))
    if (!FIELDS.has(key)) errors.push(`unknown field ${key}`);
  if (!TYPES.includes(front.type))
    errors.push(`type must be one of ${TYPES.join(", ")}`);
  if (!isList(front.tickets, (t) => TICKET.test(t)))
    errors.push("tickets must be a non-empty list of ids like mt08.1qaz");
  if ("issues" in front) errors.push(...issueErrors(front.issues));
  if (!isList(front.components, (c) => COMPONENTS.includes(c)))
    errors.push(
      `components must be a non-empty list from ${COMPONENTS.join(", ")}`,
    );
  if (front.type === "fix") {
    const v = front.introduced;
    if (!(v === "cycle" || (typeof v === "string" && RELEASED.test(v))))
      errors.push("fix needs introduced: cycle or a released version X.Y.Z");
  } else if ("introduced" in front) {
    errors.push(`introduced is only allowed on fix, not on ${front.type}`);
  }
  // Own keys only: `type: constructor` must not find Object's constructor.
  const required = Object.hasOwn(SECTIONS, front.type)
    ? SECTIONS[front.type]
    : [];
  for (const name of required)
    if (!sections.includes(name)) errors.push(`missing section ## ${name}`);
  for (const name of sections)
    if (!required.includes(name)) errors.push(`unexpected section ## ${name}`);
  if (front.type === "internal") {
    const lines = body.split("\n").filter((line) => line.trim() !== "");
    if (lines.length !== 1)
      errors.push("internal has a one-line body and no sections");
  }
  return errors;
}

const isFragmentName = (name) => name.endsWith(".md") && name !== "README.md";

export function listFragments(dir = "changes") {
  return readdirSync(dir)
    .filter(isFragmentName)
    .sort()
    .map((name) => ({ slug: name.slice(0, -3), file: join(dir, name) }));
}

/**
 * The fragments among repository paths (`dir/<slug>.md` directly in `dir`,
 * README.md excepted), sorted by slug, in the shape listFragments returns.
 */
export function fragmentsIn(paths, dir = "changes") {
  const prefix = `${dir}/`;
  return paths
    .filter((path) => path.startsWith(prefix))
    .map((path) => path.slice(prefix.length))
    .filter((name) => !name.includes("/") && isFragmentName(name))
    .sort()
    .map((name) => ({ slug: name.slice(0, -3), file: `${prefix}${name}` }));
}

/**
 * Every fragment as it stands at the commit `ref`, read with git, never
 * from the working tree: { slug, file, errors } and, when errors is empty,
 * the parsed { front, body, sections }. `opts` are git's (cwd, env).
 */
export function fragmentsAt(ref, opts) {
  return fragmentsIn(filesAt(ref, "changes", opts)).map(({ slug, file }) => {
    const text = fileAt(ref, file, opts);
    const errors = validateFragment(slug, text);
    return errors.length > 0
      ? { slug, file, errors }
      : { slug, file, errors, ...parseFragment(text) };
  });
}

export function readFragments(dir = "changes") {
  return listFragments(dir).map(({ slug, file }) => {
    const text = readFileSync(file, "utf8");
    const errors = validateFragment(slug, text);
    if (errors.length > 0) throw new Error(`${file}: ${errors[0]}`);
    return { slug, file, ...parseFragment(text) };
  });
}

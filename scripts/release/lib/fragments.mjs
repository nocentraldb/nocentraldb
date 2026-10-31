// Change fragments (changes/<slug>.md): YAML front matter between `---`
// lines, then a Markdown body whose level-2 headings are the sections.
// The rules: docs/release-management.md, "Writing a change fragment".
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { parse } from "yaml";

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
const FIELDS = new Set(["type", "tickets", "introduced", "components"]);

export function parseFragment(text) {
  const match = /^---\n([\s\S]*?)\n---\n?([\s\S]*)$/.exec(text);
  if (!match) throw new Error("no front matter block (--- … ---) at the top");
  const front = parse(match[1]);
  if (front === null || typeof front !== "object" || Array.isArray(front)) {
    throw new Error("front matter is not a mapping");
  }
  const body = match[2];
  return { front, body, sections: headings(body) };
}

// The level-2 headings of a Markdown body, outside fenced code blocks (``` or
// ~~~), so a `## ` line in an example is not taken for a section.
function headings(body) {
  const found = [];
  let fence = null;
  for (const line of body.split("\n")) {
    const marker = /^ {0,3}(`{3,}|~{3,})/.exec(line)?.[1];
    if (fence === null && marker) fence = marker;
    else if (
      fence !== null &&
      marker?.[0] === fence[0] &&
      marker.length >= fence.length &&
      line.trim() === marker
    )
      fence = null;
    else if (fence === null) {
      const heading = /^## (.+?)\s*$/.exec(line);
      if (heading) found.push(heading[1]);
    }
  }
  return found;
}

const isList = (value, ok) =>
  Array.isArray(value) &&
  value.length > 0 &&
  value.every((v) => typeof v === "string" && ok(v));

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
  const required = SECTIONS[front.type] ?? [];
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

export function listFragments(dir = "changes") {
  return readdirSync(dir)
    .filter((name) => name.endsWith(".md") && name !== "README.md")
    .sort()
    .map((name) => ({ slug: name.slice(0, -3), file: join(dir, name) }));
}

export function readFragments(dir = "changes") {
  return listFragments(dir).map(({ slug, file }) => {
    const text = readFileSync(file, "utf8");
    const errors = validateFragment(slug, text);
    if (errors.length > 0) throw new Error(`${file}: ${errors[0]}`);
    return { slug, file, ...parseFragment(text) };
  });
}

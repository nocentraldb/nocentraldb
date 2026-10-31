// Fails when a tool or npm dependency is not pinned to one exact version:
// - .prototools must pin node and pnpm;
// - .nvmrc must equal the node pin in .prototools;
// - packageManager in the root package.json must equal pnpm@<pnpm pin>;
// - every package.json dependency must be an exact version (x.y.z with an
//   optional prerelease/build suffix) or a workspace: reference to a project
//   in this repository. Ranges, tags, URLs, git, file: and npm: aliases fail;
// - so must every value in pnpm.overrides and resolutions of the root
//   package.json, and in overrides, catalog and catalogs of
//   pnpm-workspace.yaml, since a range there bypasses the rule above;
// - a catalog: reference in a dependency field must name an entry of the
//   catalog it points at (that entry is checked exact).
// Node built-ins only, so it runs before `pnpm install`. pnpm-workspace.yaml
// is therefore read by a narrow parser: the three sections above must be
// block mappings of plain or quoted scalars; anything else in them (flow
// collections, anchors, aliases, tags, block scalars, escapes, continuation
// lines) fails as unparseable instead of being skipped. Paths are relative
// to the working directory, the repository root (scripts/check-pins.test.mjs
// runs it in fixture trees).
import { globSync, readFileSync, statSync } from "node:fs";

const fail = (message) => {
  console.error(`check-pins: ${message}`);
  process.exitCode = 1;
};

const read = (file) => {
  try {
    return readFileSync(file, "utf8");
  } catch (error) {
    fail(`cannot read ${file}: ${error.code ?? error.message}`);
    return null;
  }
};

const readJson = (file) => {
  const text = read(file);
  if (text === null) return null;
  try {
    return JSON.parse(text);
  } catch (error) {
    fail(`${file} is not valid JSON: ${error.message}`);
    return null;
  }
};

const isFile = (file) => {
  try {
    return statSync(file).isFile();
  } catch {
    return false;
  }
};

const EXACT = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/;
const WORKSPACE = /^workspace:/;

const proto = Object.fromEntries(
  (read(".prototools") ?? "")
    .split("\n")
    .map((line) => line.match(/^(\w+)\s*=\s*"([^"]+)"/))
    .filter(Boolean)
    .map((match) => [match[1], match[2]]),
);
for (const key of ["node", "pnpm"]) {
  if (!proto[key]) fail(`.prototools has no ${key} pin`);
}

const nvmrc = read(".nvmrc")?.trim();
if (nvmrc !== undefined && proto.node && nvmrc !== proto.node) {
  fail(`.nvmrc ${nvmrc} != .prototools node ${proto.node}`);
}

const root = readJson("package.json");
if (root && proto.pnpm && root.packageManager !== `pnpm@${proto.pnpm}`) {
  fail(
    `package.json packageManager ${root.packageManager} != pnpm@${proto.pnpm} from .prototools`,
  );
}

// --- pnpm-workspace.yaml: the overrides, catalog and catalogs sections ---

const WORKSPACE_FILE = "pnpm-workspace.yaml";
const SECTIONS = new Set(["overrides", "catalog", "catalogs"]);

// `key:` or `key: value`; the key plain, "double" or 'single' quoted.
const ENTRY =
  /^("[^"\\]*"|'(?:[^']|'')*'|[^\s"'&*!|>%@`{}[\],?:#-][^#]*?|-[^\s#][^#]*?)\s*:(?:\s+(.*))?$/;
const COMMENT = /^(?:\s+#.*)?$/;

const unquote = (text) =>
  text.startsWith('"')
    ? text.slice(1, -1)
    : text.startsWith("'")
      ? text.slice(1, -1).replaceAll("''", "'")
      : text;

// Prototype-less, so a key named __proto__ is an entry like any other.
const mapping = () => Object.create(null);

/** A value: null when empty, {} for `{}`, a string, or undefined when unparseable. */
const scalar = (text) => {
  if (text === undefined || COMMENT.test(` ${text}`)) return null;
  const quoted = text.match(/^("[^"\\]*"|'(?:[^']|'')*')(.*)$/);
  if (quoted) return COMMENT.test(quoted[2]) ? unquote(quoted[1]) : undefined;
  const flowEmpty = text.match(/^\{\}(.*)$/);
  if (flowEmpty) return COMMENT.test(flowEmpty[1]) ? mapping() : undefined;
  if (/^["'&*!|>%@`{}[\],?#]|^[-:?](\s|$)/.test(text)) return undefined;
  const plain = text.replace(/\s+#.*$/, "");
  return /:(\s|$)/.test(plain) ? undefined : plain;
};

/** Returns { overrides, catalog, catalogs } as nested objects, or null. */
const readWorkspace = () => {
  if (!isFile(WORKSPACE_FILE)) return null;
  const text = read(WORKSPACE_FILE);
  if (text === null) return null;
  const sections = mapping();
  // The frames of the section being read: undefined outside a section,
  // [] after `section: {}` (no entries may follow), else one per open key.
  let stack;
  let broken = false;
  const unparseable = (number, line) => {
    fail(
      `${WORKSPACE_FILE}:${number}: cannot parse ${JSON.stringify(line)}; ` +
        "overrides, catalog and catalogs must be block mappings of plain or quoted scalars",
    );
    broken = true;
    stack = undefined;
  };
  for (const [index, line] of text.split(/\r?\n/).entries()) {
    const number = index + 1;
    if (/^\s*(#.*)?$/.test(line) || (line === "---" && number === 1)) continue;
    const indent = line.match(/^ */)[0].length;
    if (indent === 0 && !/^-(\s|$)/.test(line)) {
      // A top-level line: an entry, or something that could hide a section.
      const entry = line.match(ENTRY);
      if (!entry) {
        unparseable(number, line);
        break;
      }
      const key = unquote(entry[1]);
      if (!SECTIONS.has(key)) {
        stack = undefined;
        continue;
      }
      const value = scalar(entry[2]);
      if (
        value === undefined ||
        typeof value === "string" ||
        Object.hasOwn(sections, key)
      ) {
        unparseable(number, line);
        break;
      }
      sections[key] = value;
      stack = value === null ? [{ indent: 0, holder: sections, key }] : [];
      continue;
    }
    if (stack === undefined) continue; // inside a section check-pins ignores
    const entry = line.slice(indent).match(ENTRY);
    if (line.includes("\t") || indent === 0 || !entry || stack.length === 0) {
      unparseable(number, line);
      break;
    }
    while (stack.length > 1 && indent <= stack.at(-1).indent) stack.pop();
    const parent = stack.at(-1);
    parent.childIndent ??= indent;
    const value = scalar(entry[2]);
    parent.holder[parent.key] ??= mapping();
    const map = parent.holder[parent.key];
    const key = unquote(entry[1]);
    if (
      indent !== parent.childIndent ||
      value === undefined ||
      Object.hasOwn(map, key)
    ) {
      unparseable(number, line);
      break;
    }
    map[key] = value;
    if (value === null) stack.push({ indent, holder: map, key });
  }
  return broken ? null : sections;
};

const checkExact = (where, map) => {
  for (const [name, spec] of Object.entries(map ?? {})) {
    const text = typeof spec === "string" ? spec : "";
    if (!EXACT.test(text) && !WORKSPACE.test(text)) {
      fail(`${where}.${name} = "${text}" is not exact`);
    }
  }
};

const workspace = readWorkspace();
if (workspace) {
  checkExact(`${WORKSPACE_FILE} overrides`, workspace.overrides);
  checkExact(`${WORKSPACE_FILE} catalog`, workspace.catalog);
  for (const [name, catalog] of Object.entries(workspace.catalogs ?? {})) {
    if (catalog === null || typeof catalog !== "object") {
      fail(
        `${WORKSPACE_FILE} catalogs.${name} is not a mapping of names to versions`,
      );
    } else {
      checkExact(`${WORKSPACE_FILE} catalogs.${name}`, catalog);
    }
  }
}
if (root) {
  checkExact("package.json pnpm.overrides", root.pnpm?.overrides);
  checkExact("package.json resolutions", root.resolutions);
}

const CATALOG = /^catalog:(.*)$/;
const catalogEntry = (name, dep) => {
  const catalog =
    name === "default"
      ? (workspace?.catalog ?? workspace?.catalogs?.default)
      : workspace?.catalogs?.[name];
  return (
    catalog !== null &&
    typeof catalog === "object" &&
    Object.hasOwn(catalog, dep)
  );
};

const fields = [
  "dependencies",
  "devDependencies",
  "peerDependencies",
  "optionalDependencies",
];
for (const file of [
  "package.json",
  ...globSync("{apps,packages}/*/package.json").sort(),
]) {
  const pkg = file === "package.json" ? root : readJson(file);
  if (!pkg) continue;
  for (const field of fields) {
    for (const [dep, spec] of Object.entries(pkg[field] ?? {})) {
      const catalog = spec.match(CATALOG);
      if (catalog) {
        if (!catalogEntry(catalog[1] || "default", dep)) {
          fail(`${file} ${field}.${dep} = "${spec}" names no catalog entry`);
        }
      } else if (!EXACT.test(spec) && !WORKSPACE.test(spec)) {
        fail(`${file} ${field}.${dep} = "${spec}" is not exact`);
      }
    }
  }
}

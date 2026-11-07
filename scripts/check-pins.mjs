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
//   catalog it points at (that entry is checked exact);
// - every registry dependency of every Cargo.toml in the repository (outside
//   target/ and node_modules/), in [dependencies], [dev-dependencies],
//   [build-dependencies], their target.<cfg> forms and
//   [workspace.dependencies], must be an exact `=x.y.z` requirement.
//   `name.workspace = true` passes: the workspace entry it inherits is
//   checked where it is declared. A `path =` entry is left to
//   scripts/release/check-version.mjs, which pins it to the workspace
//   version. Git dependencies and [patch] and [replace] sections fail.
// Cargo.toml is read with the pinned smol-toml devDependency, so the Cargo
// part needs `pnpm install` (`pnpm gates` runs after it). The npm part uses
// Node built-ins only. pnpm-workspace.yaml is read by a narrow parser: the three sections above must be
// block mappings of plain or quoted scalars; anything else in them (flow
// collections, anchors, aliases, tags, block scalars, escapes, continuation
// lines) fails as unparseable instead of being skipped. Paths are relative
// to the working directory, the repository root (scripts/check-pins.test.mjs
// runs it in fixture trees).
import { globSync, readFileSync, statSync } from "node:fs";
import { parse as parseToml } from "smol-toml";

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

// --- Cargo.toml: every registry dependency pinned to =x.y.z ---

const CARGO_EXACT = /^=\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;
const CARGO_TABLES = [
  "dependencies",
  "dev-dependencies",
  "build-dependencies",
  "dev_dependencies",
  "build_dependencies",
];

const readToml = (file) => {
  const text = read(file);
  if (text === null) return null;
  try {
    return parseToml(text);
  } catch (error) {
    fail(`${file} is not valid TOML: ${error.message.split("\n")[0]}`);
    return null;
  }
};

const checkCargoTable = (file, where, table) => {
  for (const [dep, spec] of Object.entries(table ?? {})) {
    const entry = `${file} ${where}.${dep}`;
    if (typeof spec === "string") {
      if (!CARGO_EXACT.test(spec)) {
        fail(`${entry} = "${spec}" is not exact (use "=x.y.z")`);
      }
    } else if (spec?.git !== undefined) {
      fail(
        `${entry} is a git dependency; use a registry release pinned to "=x.y.z"`,
      );
    } else if (spec?.workspace === true || spec?.path !== undefined) {
      // Inherited from [workspace.dependencies], which is checked itself; or
      // a path dependency between workspace crates, whose rule (version =
      // "=<workspace version>") scripts/release/check-version.mjs enforces.
    } else if (typeof spec?.version !== "string") {
      fail(`${entry} has no version (use version = "=x.y.z")`);
    } else if (!CARGO_EXACT.test(spec.version)) {
      fail(`${entry} = "${spec.version}" is not exact (use "=x.y.z")`);
    }
  }
};

const cargoFiles = globSync("**/Cargo.toml", {
  exclude: (path) => /(^|\/)(target|node_modules|\.git)$/.test(path),
}).sort();
for (const file of cargoFiles) {
  const manifest = readToml(file);
  if (!manifest) continue;
  for (const section of ["patch", "replace"]) {
    if (manifest[section] !== undefined) {
      fail(
        `${file} has a [${section}] section; it replaces pinned dependencies and is not allowed`,
      );
    }
  }
  checkCargoTable(
    file,
    "workspace.dependencies",
    manifest.workspace?.dependencies,
  );
  for (const name of CARGO_TABLES) {
    checkCargoTable(file, name, manifest[name]);
  }
  for (const [cfg, target] of Object.entries(manifest.target ?? {})) {
    for (const name of CARGO_TABLES) {
      checkCargoTable(file, `target.${cfg}.${name}`, target?.[name]);
    }
  }
}

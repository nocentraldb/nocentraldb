// Narrow, fail-closed readers and writers for the manifests that carry the
// version (docs/release-management.md, "Versioning"): the workspace
// Cargo.toml, the crate manifests crates/*/Cargo.toml, Cargo.lock and the
// workspace package.json files. Not a TOML parser: the reader walks lines and
// tracks the current [table] header. Inside the tables it reads it accepts
// only single-line key/value pairs of the shapes it knows; anything else
// there (a multi-line value, a trailing comment on a line it parses, a dotted
// dependency key, a dependency sub-table such as [workspace.dependencies.x])
// throws "unsupported manifest syntax at line N", in the style of the
// pnpm-workspace parser of scripts/check-pins.mjs. In every other table a
// multi-line value is stepped over, so a header-looking line inside it is
// never taken for a header.
import { globSync } from "node:fs";

const HEADER = /^\s*(\[\[?)\s*([^\]]+?)\s*\]\]?\s*(?:#.*)?$/;
const KEY_VALUE = /^\s*([A-Za-z0-9_.-]+)\s*=\s*(.*)$/;
const STRINGS = /"(?:[^"\\]|\\.)*"|'[^']*'/g;
const QUOTED = /^"([^"]*)"\s*$/;
const INLINE_TABLE = /^(\s*[A-Za-z0-9_-]+\s*=\s*\{)(.*)(\}\s*)$/;
const NAME = /^[A-Za-z0-9_-]+$/;

const unsupported = (lineNo) =>
  new Error(`unsupported manifest syntax at line ${lineNo}`);

/** Bracket depth a value opens: [ and { minus ] and }, strings and comments removed. */
const depth = (text) => {
  const bare = text.replace(STRINGS, "").replace(/#.*$/, "");
  let d = 0;
  for (const c of bare) {
    if (c === "[" || c === "{") d++;
    else if (c === "]" || c === "}") d--;
  }
  return d;
};

const multiLineString = (value) => {
  for (const q of ['"""', "'''"]) {
    if (value.startsWith(q) && value.split(q).length < 3) return q;
  }
  return null;
};

/**
 * Walks the lines of a TOML file. `reads(table)` says which tables the
 * caller parses; `rejects(table)` which headers it refuses outright. Yields
 * { index, lineNo, table, key, value } for every key/value line of a read
 * table, and throws on any line of a read table it cannot classify.
 */
function* scan(text, { reads, rejects = () => false }) {
  const lines = text.split("\n");
  let table = "";
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].replace(/\r$/, "");
    const lineNo = i + 1;
    if (/^\s*(?:#.*)?$/.test(line)) continue;
    const header = HEADER.exec(line);
    if (header) {
      table = header[1] === "[[" ? `[[${header[2]}]]` : header[2];
      if (rejects(table)) throw unsupported(lineNo);
      continue;
    }
    const kv = KEY_VALUE.exec(line);
    const reading = reads(table);
    if (!kv) {
      if (reading) throw unsupported(lineNo);
      continue;
    }
    const value = kv[2];
    const quote = multiLineString(value);
    const open = quote ? 0 : depth(value);
    if (quote || open > 0) {
      if (reading) throw unsupported(lineNo);
      // Step over the rest of the value.
      let d = open;
      while (++i < lines.length) {
        if (quote) {
          if (lines[i].includes(quote)) break;
          continue;
        }
        d += depth(lines[i]);
        if (d <= 0) break;
      }
      continue;
    }
    if (reading) yield { index: i, lineNo, table, key: kv[1], value };
  }
}

const workspacePackage = (text) => {
  let seen = false;
  const versions = [];
  for (const e of scan(text, {
    reads: (t) => {
      if (t === "workspace.package") seen = true;
      return t === "workspace.package";
    },
  })) {
    if (e.key !== "version") continue;
    const m = QUOTED.exec(e.value);
    if (!m) throw unsupported(e.lineNo);
    versions.push({ ...e, version: m[1] });
  }
  if (!seen) throw new Error("no [workspace.package] table");
  if (versions.length === 0)
    throw new Error("[workspace.package] has no version");
  if (versions.length > 1)
    throw new Error("[workspace.package] has more than one version");
  return versions[0];
};

/** The version of [workspace.package]; throws when the table or the line is missing or doubled. */
export function readWorkspaceVersion(cargoToml) {
  return workspacePackage(cargoToml).version;
}

/** cargoToml with the version line of [workspace.package] set to `version`. */
export function writeWorkspaceVersion(cargoToml, version) {
  const { index } = workspacePackage(cargoToml);
  const lines = cargoToml.split("\n");
  lines[index] = lines[index].replace(/"[^"]*"/, `"${version}"`);
  return lines.join("\n");
}

/** The `name`, `version` and `path` of an inline-table dependency, or throws. */
const dependencyEntry = (e) => {
  const inline = INLINE_TABLE.exec(e.line);
  if (!inline) throw unsupported(e.lineNo);
  const body = ` ${inline[2]}`;
  const field = (key) => {
    if (!new RegExp(`[\\s,]${key}\\s*=`).test(body)) return null;
    const m = new RegExp(`[\\s,]${key}\\s*=\\s*"([^"]*)"`).exec(body);
    if (!m) throw unsupported(e.lineNo);
    return m[1];
  };
  return {
    name: e.key,
    version: field("version"),
    path: field("path"),
    inline,
  };
};

/**
 * The entries of one kind of dependency table: plain `name = "1"`,
 * `name.workspace = true`, and single-line inline tables. Anything else
 * throws.
 */
const dependencies = (text, isTable, rejects) => {
  const lines = text.split("\n");
  const out = [];
  for (const e of scan(text, { reads: isTable, rejects })) {
    const line = lines[e.index].replace(/\r$/, "");
    const dotted = /^(.+)\.workspace$/.exec(e.key);
    if (dotted) {
      if (!NAME.test(dotted[1]) || !/^true\s*$/.test(e.value))
        throw unsupported(e.lineNo);
      continue;
    }
    if (!NAME.test(e.key)) throw unsupported(e.lineNo);
    if (e.value.startsWith("{"))
      out.push({ ...dependencyEntry({ ...e, line }), index: e.index });
    else if (!QUOTED.test(e.value)) throw unsupported(e.lineNo);
  }
  return out;
};

const workspaceDependencyEntries = (cargoToml) =>
  dependencies(
    cargoToml,
    (t) => t === "workspace.dependencies",
    (t) => t.startsWith("workspace.dependencies."),
  ).filter((d) => d.path !== null);

/** The entries of [workspace.dependencies] that have a `path =`. */
export function workspaceDependencies(cargoToml) {
  return workspaceDependencyEntries(cargoToml).map(
    ({ name, version, path }) => ({ name, version, path }),
  );
}

/** cargoToml with every path entry of [workspace.dependencies] pinned to `=<version>`. */
export function pinWorkspaceDependencies(cargoToml, version) {
  const lines = cargoToml.split("\n");
  for (const d of workspaceDependencyEntries(cargoToml)) {
    const [, head, body, tail] = d.inline;
    const pin = `version = "=${version}"`;
    const next =
      d.version === null
        ? body.replace(/^\s*/, (ws) => `${ws}${pin}, `)
        : ` ${body}`
            .replace(/([\s,])version\s*=\s*"[^"]*"/, `$1${pin}`)
            .slice(1);
    const cr = lines[d.index].endsWith("\r") ? "\r" : "";
    lines[d.index] = `${head}${next}${tail}${cr}`;
  }
  return lines.join("\n");
}

const DEPENDENCY_TABLE = /^(?:target\..+\.)?(?:dev-|build-)?dependencies$/;
const DEPENDENCY_SUBTABLE =
  /^(?:target\..+\.)?(?:dev-|build-)?dependencies\..+$/;

/**
 * What a crate manifest says about its version and its path dependencies:
 * [package] name, `version.workspace = true` or an own `version = "…"`, and
 * the names of every `path =` entry in [dependencies], [dev-dependencies],
 * [build-dependencies] and their [target.*.…] forms.
 */
export function crateManifest(text) {
  let name = null;
  let inheritsVersion = false;
  let ownVersion = null;
  let versions = 0;
  for (const e of scan(text, { reads: (t) => t === "package" })) {
    if (e.key === "name") {
      const m = QUOTED.exec(e.value);
      if (!m) throw unsupported(e.lineNo);
      name = m[1];
    } else if (e.key === "version") {
      const m = QUOTED.exec(e.value);
      if (!m) throw unsupported(e.lineNo);
      ownVersion = m[1];
      versions++;
    } else if (e.key === "version.workspace") {
      if (!/^true\s*$/.test(e.value)) throw unsupported(e.lineNo);
      inheritsVersion = true;
      versions++;
    } else if (e.key.startsWith("version.")) throw unsupported(e.lineNo);
  }
  if (versions > 1) throw new Error("[package] has more than one version");
  const pathDependencies = dependencies(
    text,
    (t) => DEPENDENCY_TABLE.test(t),
    (t) => DEPENDENCY_SUBTABLE.test(t),
  )
    .filter((d) => d.path !== null)
    .map((d) => d.name);
  return { name, inheritsVersion, ownVersion, pathDependencies };
}

/**
 * The value of `publish` in the [package] table of a crate manifest, as
 * written (`false`, `true`, `["crates-io"]`), or null when it has none.
 */
export function publishLine(text) {
  let value = null;
  for (const e of scan(text, { reads: (t) => t === "package" }))
    if (e.key === "publish") value = e.value.trim();
  return value;
}

/**
 * name -> version of every [[package]] block of a Cargo.lock. When a name
 * appears more than once, the block without a `source` (a workspace member)
 * wins.
 */
export function lockVersions(cargoLock) {
  const out = new Map();
  const blocks = [];
  let block = null;
  const lines = cargoLock.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].replace(/\r$/, "");
    if (/^\s*\[/.test(line)) {
      block = line.trim() === "[[package]]" ? { lineNo: i + 1 } : null;
      if (block) blocks.push(block);
      continue;
    }
    const m = /^(name|version|source)\s*=\s*"([^"]*)"\s*$/.exec(line);
    if (block && m) block[m[1]] = m[2];
  }
  for (const b of blocks) {
    if (b.name === undefined || b.version === undefined)
      throw new Error(
        `Cargo.lock: the [[package]] block at line ${b.lineNo} has no name or no version`,
      );
    if (!out.has(b.name) || b.source === undefined) out.set(b.name, b.version);
  }
  return out;
}

const JSON_STRING = /"(?:[^"\\]|\\.)*"/y;
const JSON_COLON = /\s*:\s*/y;

/**
 * The package.json text with the value of its top-level "version" replaced
 * by `version`, and every other byte as it was: indentation, key order,
 * spacing and the final newline stay. Walks the text keeping track of the
 * object depth, so a "version" nested deeper (devEngines) or inside a string
 * is never taken. Throws on text that is not JSON and when the top level has
 * no "version" string.
 */
export function writePackageVersion(text, version) {
  const parsed = JSON.parse(text);
  if (typeof parsed?.version !== "string")
    throw new Error('no top-level "version" string');
  let depth = 0;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === "{" || c === "[") depth++;
    else if (c === "}" || c === "]") depth--;
    else if (c === '"') {
      JSON_STRING.lastIndex = i;
      const key = JSON_STRING.exec(text)[0];
      const end = i + key.length;
      JSON_COLON.lastIndex = end;
      const colon = depth === 1 ? JSON_COLON.exec(text) : null;
      if (key === '"version"' && colon && text[end + colon[0].length] === '"') {
        const start = end + colon[0].length;
        JSON_STRING.lastIndex = start;
        const value = JSON_STRING.exec(text)[0];
        return `${text.slice(0, start)}${JSON.stringify(version)}${text.slice(start + value.length)}`;
      }
      i = end - 1;
    }
  }
  // JSON.parse found a top-level version string, so the walk finds it too.
  throw new Error('no top-level "version" string');
}

const glob = (root, patterns) =>
  patterns.flatMap((p) => globSync(p, { cwd: root })).sort();

/** packages/*\/package.json and apps/*\/package.json, relative to root, sorted. */
export function packageJsonFiles(root) {
  return glob(root, ["packages/*/package.json", "apps/*/package.json"]);
}

/** crates/*\/Cargo.toml, relative to root, sorted. */
export function crateManifestFiles(root) {
  return glob(root, ["crates/*/Cargo.toml"]);
}

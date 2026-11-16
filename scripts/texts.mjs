#!/usr/bin/env node

// Every text a developer reads from the SDKs lives in texts/texts.json
// (NOC-329; the owner's decision of 2026-11-15). This script generates the
// Rust module the SDK reads them from, crates/nocentraldb/src/texts/, one
// file per kind of text, and the one the `ncdb` command line reads its own
// from, crates/ncdb/src/texts.rs (kind `cli`, T5), the TypeScript
// SDK's, packages/sdk/src/texts.ts (kind `ts`, T3, scripts/texts-ts.mjs), and
// the core's, crates/ncdb-core/src/texts/ (kind `core`, T6,
// scripts/texts-core.mjs), and checks for the gate (task repo:texts):
//
// - the file: each entry has an id (`<kind>.<name>`), a kind, a text and when
//   it shows, and a notice or an error its code; ids are unique; a notice's
//   name is its code, a NoticeCode, and every NoticeCode has its notice; an
//   error's code names the ErrorCode it is returned with, or several,
//   separated by ` | `; a part is a piece of an error's text, with no code,
//   and so is a text of the command line, of kind `cli`, and of the core, of
//   kind `core`; a text holds no braces but its placeholders, `{name}` or
//   `{name:?}`, and a brace written as Rust writes it, doubled, `{{` or
//   `}}`, which a ts text does not hold; the
//   entries are sorted by id and the file is written one field per line, as
//   `JSON.stringify(file, null, 2)` writes it, so that `git diff` shows each
//   change to a text readably (the owner reviews the texts from git diff:
//   Carsten, 2026-11-15, NOC-329);
// - the generated module equals what this script writes from the file: a
//   constant per text, or a function that fills its placeholders;
// - no text is written in the SDK's non-test code outside the generated
//   module: a notice's `text:`, `Error::new`'s message, an `Err` or a helper
//   that builds an error, given a string literal, `format!` or `concat!`;
// - no text is written in the command line's non-test code outside its
//   module: a `&str` constant, the schema script's `fail` given a literal,
//   or any literal with two words in a row, except
//   on a line marked `// notation`, for the report's diff notation (pm,
//   NOC-329);
// - every text is used outside the generated module.
//
// `node scripts/texts.mjs --write` regenerates the module after an edit;
// `node scripts/texts.mjs --check` is the gate.
//
// `pnpm texts`, with no option, is the owner's review before a beta or a
// stable release (Carsten, 2026-11-15, NOC-329): it finds the last release
// tag itself, the newest `vX.Y.Z-beta.N` or `vX.Y.Z`, reads texts.json as it
// was at that tag with `git show`, and lists every text new, changed or
// removed since, with its kind, code and when it shows, and the old and the
// new text. With no release tag yet, every text counts as new. `--since
// <tag>` compares against another tag.

import {
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { coreGate } from "./texts-core.mjs";
import { review } from "./texts-review.mjs";
import { rfcErrorCodes, TS_PATHS, tsGate } from "./texts-ts.mjs";

export {
  diffTexts,
  formatReview,
  lastReleaseTag,
  review,
} from "./texts-review.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

/** Where the texts and the SDK live, relative to the root; tests point elsewhere. */
export const PATHS = {
  texts: "texts/texts.json",
  rust: "crates/nocentraldb/src",
  module: "crates/nocentraldb/src/texts",
  noticeCodes: "crates/nocentraldb/src/members/notice.rs",
  errorCodes: "crates/nocentraldb/src/error.rs",
  cli: "crates/ncdb/src",
  cliModule: "crates/ncdb/src/texts.rs",
};

const KINDS = ["notice", "error", "part", "cli", "ts", "core"];
/** The kinds the SDK's module holds; `cli` is the command line's. */
const SDK_KINDS = ["notice", "error", "part"];
/** The kinds without a code. */
const CODELESS = ["part", "cli", "core"];
const FIELDS = ["id", "kind", "code", "text", "when"];
const NAME = /^[a-z][A-Za-z0-9]*$/;
const PLACEHOLDER = /\{(\w+)(:\?)?\}/g;
/** A brace written as Rust's format! writes it, doubled: `{{` or `}}`. */
const ESCAPED = /\{\{|\}\}/g;

/** `text` with its doubled braces single, as a constant holds it. */
const unescaped = (text) => text.replace(/\{\{/g, "{").replace(/\}\}/g, "}");

/** `memberAdded` → `MEMBER_ADDED`. */
export const constName = (name) =>
  name.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toUpperCase();

/** `noSuchSpace` → `no_such_space`. */
export const fnName = (name) => constName(name).toLowerCase();

/** `memberAdded` → `MemberAdded`, the Rust variant. */
const variant = (code) => code[0].toUpperCase() + code.slice(1);

/** The name of an entry: its id after the kind. */
const nameOf = (entry) => entry.id.slice(entry.kind.length + 1);

/** The placeholders of `text`, each once, in order, with whether it is Debug. */
export const placeholders = (text) => {
  const seen = new Map();
  // A doubled brace is no placeholder's; blanking it keeps the offsets.
  for (const [, name, debug] of text
    .replace(ESCAPED, "  ")
    .matchAll(PLACEHOLDER)) {
    if (!seen.has(name)) seen.set(name, Boolean(debug));
  }
  return [...seen];
};

/** A Rust string literal of `text`. */
const rustString = (text) => {
  let out = '"';
  for (const c of text) {
    const n = c.codePointAt(0);
    if (c === '"') out += '\\"';
    else if (c === "\\") out += "\\\\";
    else if (n < 0x20 || n === 0x7f) out += `\\u{${n.toString(16)}}`;
    else out += c;
  }
  return `${out}"`;
};

/** A doc comment of `text`, wrapped at 76 columns after `prefix`. */
const doc = (text, prefix) => {
  const lines = [];
  let line = "";
  for (const word of text.split(/\s+/)) {
    if (line && line.length + 1 + word.length > 76) {
      lines.push(line);
      line = word;
    } else line = line ? `${line} ${word}` : word;
  }
  if (line) lines.push(line);
  return lines.map((l) => `${prefix}/// ${l}`).join("\n");
};

const MODULES = {
  notice: "The texts of [`crate::Notice`], by code.",
  error: "The messages of [`crate::Error`], by name.",
  part: "Pieces of the messages of [`crate::Error`], such as a reason.",
  bridge:
    "The bridge's own messages of [`crate::Error`], for a request the TypeScript SDK sent malformed (T3).",
};

/** The bridge's errors, `error.bridge<Name>`, have a file of their own (T3). */
const BRIDGE = /^bridge[A-Z]/;

/** The generated file of `entry`: its kind's, or `bridge` for the bridge's errors. */
const fileOf = (entry) =>
  entry.kind === "error" && BRIDGE.test(nameOf(entry)) ? "bridge" : entry.kind;

/** The name `entry` has in its file: a bridge error's without `bridge`. */
const itemName = (entry) => {
  const name = nameOf(entry);
  return fileOf(entry) === "bridge"
    ? name[6].toLowerCase() + name.slice(7)
    : name;
};

const GENERATED = [
  "// Generated by scripts/texts.mjs from texts/texts.json: do not edit.",
  "// Change a text there, then run `node scripts/texts.mjs --write`.",
  "",
];

/**
 * The Rust module the texts generate, as { file name: content }: mod.rs,
 * and one file per kind that has texts.
 */
export const generate = (texts) => {
  const files = {};
  const kinds = [...SDK_KINDS, "bridge"].filter((kind) =>
    texts.some((t) => SDK_KINDS.includes(t.kind) && fileOf(t) === kind),
  );
  files["mod.rs"] = [
    ...GENERATED,
    "//! Every text a developer reads from the SDK, from texts/texts.json",
    "//! (NOC-329), one file per kind. Each text's comment says when it shows.",
    "",
    ...kinds.flatMap((kind) => [
      `/// ${MODULES[kind]}`,
      // The browser build and the native one each use only their own texts.
      "#[allow(dead_code)]",
      `pub(crate) mod ${kind};`,
    ]),
  ]
    .join("\n")
    .concat("\n");
  for (const kind of kinds) {
    const out = [...GENERATED, `//! ${MODULES[kind]}`];
    texts
      .filter((t) => SDK_KINDS.includes(t.kind) && fileOf(t) === kind)
      .forEach((entry) => {
        out.push("");
        // A notice's name is its code.
        const code = entry.kind === "error" ? ` (${entry.code})` : "";
        out.push(doc(`\`${nameOf(entry)}\`${code}: ${entry.when}`, ""));
        const params = placeholders(entry.text);
        if (params.length === 0) {
          out.push(`pub(crate) const ${constName(itemName(entry))}: &str =`);
          out.push(`    ${rustString(unescaped(entry.text))};`);
        } else {
          const args = params
            .map(
              ([p, debug]) =>
                `${p}: impl std::fmt::${debug ? "Debug" : "Display"}`,
            )
            .join(", ");
          out.push(
            `pub(crate) fn ${fnName(itemName(entry))}(${args}) -> String {`,
          );
          out.push(`    format!(${rustString(entry.text)})`);
          out.push("}");
        }
      });
    files[`${kind}.rs`] = `${out.join("\n")}\n`;
  }
  return files;
};

/** A text's constant, or its function that fills its placeholders. */
const item = (entry) => {
  const params = placeholders(entry.text);
  if (params.length === 0) {
    return [
      `pub(crate) const ${constName(nameOf(entry))}: &str =`,
      `    ${rustString(unescaped(entry.text))};`,
    ];
  }
  const args = params
    .map(([p, debug]) => `${p}: impl std::fmt::${debug ? "Debug" : "Display"}`)
    .join(", ");
  return [
    `pub(crate) fn ${fnName(nameOf(entry))}(${args}) -> String {`,
    `    format!(${rustString(entry.text)})`,
    "}",
  ];
};

/**
 * A Rust module of the texts of one `kind`, a constant or a function per
 * text, below the `header` lines of its module comment.
 */
export const rustModule = (texts, kind, header) => {
  const out = [...GENERATED, ...header.map((line) => `//! ${line}`)];
  for (const entry of texts.filter((t) => t.kind === kind)) {
    out.push(
      "",
      doc(`\`${nameOf(entry)}\`: ${entry.when}`, ""),
      ...item(entry),
    );
  }
  return `${out.join("\n")}\n`;
};

/** The command line's module, crates/ncdb/src/texts.rs, of the `cli` texts. */
export const generateCli = (texts) =>
  rustModule(texts, "cli", [
    "Every text the `ncdb` command line shows, from texts/texts.json",
    "(NOC-329, T5). Each text's comment says when it shows.",
  ]);

/** The variants of `pub enum <name>` in `source`. */
export const enumVariants = (source, name) => {
  const body = source.match(
    new RegExp(`pub enum ${name} \\{([\\s\\S]*?)\\n\\}`),
  );
  if (!body) return [];
  return body[1]
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => /^[A-Z][A-Za-z0-9]*,$/.test(line))
    .map((line) => line.slice(0, -1));
};

/** The variants of `pub enum NoticeCode` in `source`. */
export const noticeVariants = (source) => enumVariants(source, "NoticeCode");

/** The problems of the file's entries, each a line. */
export const checkEntries = (
  file,
  variants,
  errorVariants = [],
  tsCodes = [],
) => {
  const problems = [];
  if (!file || !Array.isArray(file.texts)) {
    return ['texts.json: a "texts" array is required'];
  }
  const ids = new Set();
  for (const [i, entry] of file.texts.entries()) {
    const at = `texts[${i}]${entry?.id ? ` (${entry.id})` : ""}`;
    if (!entry || typeof entry !== "object") {
      problems.push(`${at}: not an object`);
      continue;
    }
    // A ts text has a code when the SDK throws it as an nc.Error (T3).
    const needed =
      CODELESS.includes(entry.kind) ||
      (entry.kind === "ts" && !("code" in entry))
        ? FIELDS.filter((f) => f !== "code")
        : FIELDS;
    for (const field of needed) {
      if (typeof entry[field] !== "string" || entry[field].trim() === "") {
        problems.push(`${at}: "${field}" must be a non-empty text`);
      }
    }
    for (const field of Object.keys(entry)) {
      if (!FIELDS.includes(field))
        problems.push(`${at}: unknown field "${field}"`);
    }
    if (CODELESS.includes(entry.kind) && "code" in entry) {
      problems.push(`${at}: a ${entry.kind} text has no code`);
    }
    if (typeof entry.id === "string") {
      if (ids.has(entry.id)) problems.push(`${at}: the id is used twice`);
      ids.add(entry.id);
    }
    if (typeof entry.kind !== "string" || !KINDS.includes(entry.kind)) {
      problems.push(`${at}: kind must be one of ${KINDS.join(", ")}`);
      continue;
    }
    if (typeof entry.id === "string") {
      const name = entry.id.startsWith(`${entry.kind}.`) ? nameOf(entry) : "";
      if (!NAME.test(name)) {
        problems.push(
          `${at}: the id must be "${entry.kind}." and a camelCase name`,
        );
      }
      if (entry.kind === "notice" && name !== entry.code) {
        problems.push(`${at}: a notice's id must be "notice.${entry.code}"`);
      }
    }
    if (entry.kind === "notice" && typeof entry.code === "string") {
      if (!variants.includes(variant(entry.code))) {
        problems.push(
          `${at}: NoticeCode has no variant ${variant(entry.code)}`,
        );
      }
    }
    if (entry.kind === "error" && typeof entry.code === "string") {
      for (const code of entry.code.split(" | ")) {
        if (!NAME.test(code) || !errorVariants.includes(variant(code))) {
          problems.push(`${at}: ErrorCode has no variant ${code}`);
        }
      }
    }
    if (entry.kind === "ts" && typeof entry.code === "string") {
      if (!tsCodes.includes(entry.code))
        problems.push(`${at}: RFC 0009 lists no error code ${entry.code}`);
    }
    if (typeof entry.text === "string") {
      if (entry.kind === "ts" && entry.text.includes(":?}"))
        problems.push(`${at}: a ts text has no Debug placeholder {name:?}`);
      // TypeScript's module does not read Rust's doubled braces.
      if (entry.kind === "ts" && /\{\{|\}\}/.test(entry.text))
        problems.push(`${at}: a ts text has no doubled brace {{ or }}`);
      if (
        entry.text.replace(ESCAPED, "").replace(PLACEHOLDER, "").match(/[{}]/)
      ) {
        problems.push(
          `${at}: a brace that is no placeholder {name} or {name:?}`,
        );
      }
      if (entry.kind === "notice" && placeholders(entry.text).length > 0) {
        problems.push(`${at}: a notice has no placeholders`);
      }
    }
  }
  const notices = new Set(
    file.texts.filter((t) => t?.kind === "notice").map((t) => t.code),
  );
  for (const v of variants) {
    const code = v[0].toLowerCase() + v.slice(1);
    if (!notices.has(code))
      problems.push(`NoticeCode::${v} has no notice in texts.json`);
  }
  return problems;
};

/**
 * The problems of the file's layout: entries sorted by id, and the file as
 * `JSON.stringify(file, null, 2)` writes it, one field per line, so that a
 * diff shows each changed field on a line of its own.
 */
export const checkLayout = (raw, file) => {
  const problems = [];
  const ids = (file?.texts ?? []).map((t) => t?.id);
  const sorted = [...ids].sort();
  const first = ids.findIndex((id, i) => id !== sorted[i]);
  if (first >= 0) {
    problems.push(
      `texts.json: the entries must be sorted by id; ${ids[first]} is out of order`,
    );
  }
  if (raw !== `${JSON.stringify(file, null, 2)}\n`) {
    problems.push(
      "texts.json: write it as JSON.stringify(file, null, 2) does, one field per line, with a final newline",
    );
  }
  return problems;
};

/** Every non-test .rs file under `dir`, as [path, source]. */
export const rustFiles = (dir) => {
  const out = [];
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      if (!["tests", "relay_tests"].includes(name))
        out.push(...rustFiles(path));
    } else if (name.endsWith(".rs") && !/(^|_)tests?\.rs$/.test(name)) {
      out.push([path, readFileSync(path, "utf8")]);
    }
  }
  return out;
};

/** A literal text, a `format!` or a `concat!`. */
const LITERAL = String.raw`(?:"|format!|concat!)`;

/** Texts written in code, each with what it is. */
const STRAYS = [
  [new RegExp(String.raw`\btext:\s*${LITERAL}`, "g"), "a notice text"],
  [
    new RegExp(
      String.raw`\bError::new\(\s*[^,()]+(?:\([^()]*\))?\s*,\s*${LITERAL}`,
      "g",
    ),
    "an error message",
  ],
  [new RegExp(String.raw`\bErr\(\s*${LITERAL}`, "g"), "an error's reason"],
  [
    new RegExp(
      String.raw`\b(?:invalid|failed|io|refused|holds|crypto)\([^()"]*${LITERAL}`,
      "g",
    ),
    "a piece of an error message",
  ],
];

/** How code names a text of the SDK: `<kind>::NAME` or `<kind>::name(`. */
const sdkUse = (entry) =>
  placeholders(entry.text).length === 0
    ? `${fileOf(entry)}::${constName(itemName(entry))}`
    : `${fileOf(entry)}::${fnName(itemName(entry))}(`;

/** Texts written in the command line's code, each with what it is. */
export const CLI_STRAYS = [
  [/:\s*&str\s*=\s*"/g, "a text constant"],
  [/\bfail\(\s*["`]/g, "a message of the schema script"],
  [/"(?:[^"\\\n]|\\.)*?[A-Za-z]{2,} [A-Za-z]{2,}(?:[^"\\\n]|\\.)*"/g, "a text"],
];

/**
 * How the command line names a text: `texts::NAME`, or `texts::name`
 * called or passed as a function, as `map_err(texts::name)` does.
 */
export const cliUse = (entry) =>
  placeholders(entry.text).length === 0
    ? `texts::${constName(nameOf(entry))}`
    : new RegExp(String.raw`\btexts::${fnName(nameOf(entry))}\b`);

/**
 * Blanks out what the stray check does not read: comment lines, and lines
 * marked `// notation`, the report's diff notation (pm, NOC-329).
 */
const readable = (source) =>
  source
    .split("\n")
    .map((line) =>
      /^\s*\/\//.test(line) || /\/\/ notation\b/.test(line) ? "" : line,
    )
    .join("\n");

/**
 * The problems of code: stray texts, and texts no code uses. `module` is
 * the generated module's path, a folder or a file; `strays` what counts as
 * a text written in code; `use` how code names a text.
 */
export const checkCode = (
  files,
  module,
  texts,
  { strays = STRAYS, use = sdkUse, where = "crate::texts" } = {},
) => {
  const problems = [];
  const code = files.filter(
    ([path]) => path !== module && !path.startsWith(`${module}/`),
  );
  for (const [path, original] of code) {
    const source = strays === STRAYS ? original : readable(original);
    for (const [pattern, what] of strays) {
      for (const match of source.matchAll(pattern)) {
        const line = source.slice(0, match.index).split("\n").length;
        problems.push(
          `${path}:${line}: ${what} written in code; add it to texts/texts.json and use ${where}`,
        );
      }
    }
  }
  const all = code.map(([, source]) => source).join("\n");
  for (const entry of texts) {
    const used = use(entry);
    const found = used instanceof RegExp ? used.test(all) : all.includes(used);
    if (!found)
      problems.push(
        `${entry.id}: no code uses ${String(used).startsWith("texts::") || used instanceof RegExp ? String(used) : `texts::${used}`}`,
      );
  }
  return problems;
};

const main = () => {
  const { values } = parseArgs({
    options: {
      check: { type: "boolean" },
      write: { type: "boolean" },
      since: { type: "string" },
    },
  });
  if (!values.check && !values.write) {
    process.stdout.write(review(ROOT, values.since));
    return;
  }
  const at = (path) => join(ROOT, path);
  const raw = readFileSync(at(PATHS.texts), "utf8");
  const file = JSON.parse(raw);
  const variants = noticeVariants(readFileSync(at(PATHS.noticeCodes), "utf8"));
  const errorVariants = enumVariants(
    readFileSync(at(PATHS.errorCodes), "utf8"),
    "ErrorCode",
  );
  const problems = [
    ...checkEntries(
      file,
      variants,
      errorVariants,
      rfcErrorCodes(readFileSync(at(TS_PATHS.rfc), "utf8")),
    ),
    ...checkLayout(raw, file),
  ];
  if (problems.length === 0) {
    const generated = generate(file.texts);
    const dir = at(PATHS.module);
    if (values.write) {
      mkdirSync(dir, { recursive: true });
      for (const name of readdirSync(dir)) {
        if (!(name in generated)) rmSync(join(dir, name));
      }
      for (const [name, content] of Object.entries(generated)) {
        writeFileSync(join(dir, name), content);
      }
    }
    let present = [];
    try {
      present = readdirSync(dir);
    } catch {}
    const names = new Set([...present, ...Object.keys(generated)]);
    for (const name of [...names].sort()) {
      let committed = null;
      try {
        committed = readFileSync(join(dir, name), "utf8");
      } catch {}
      if (committed !== (generated[name] ?? null)) {
        problems.push(
          `${PATHS.module}/${name} differs from texts/texts.json: run node scripts/texts.mjs --write`,
        );
      }
    }
    const files = rustFiles(at(PATHS.rust)).map(([p, s]) => [
      relative(ROOT, p),
      s,
    ]);
    const sdk = file.texts.filter((t) => SDK_KINDS.includes(t.kind));
    problems.push(...checkCode(files, PATHS.module, sdk));
    // The command line's own module, one file.
    const cli = generateCli(file.texts);
    if (values.write) writeFileSync(at(PATHS.cliModule), cli);
    let committedCli = null;
    try {
      committedCli = readFileSync(at(PATHS.cliModule), "utf8");
    } catch {}
    if (committedCli !== cli) {
      problems.push(
        `${PATHS.cliModule} differs from texts/texts.json: run node scripts/texts.mjs --write`,
      );
    }
    const cliFiles = rustFiles(at(PATHS.cli)).map(([p, s]) => [
      relative(ROOT, p),
      s,
    ]);
    problems.push(
      ...checkCode(
        cliFiles,
        PATHS.cliModule,
        file.texts.filter((t) => t.kind === "cli"),
        { strays: CLI_STRAYS, use: cliUse, where: "crate::texts" },
      ),
    );
    // The TypeScript SDK's module and code (T3, scripts/texts-ts.mjs).
    problems.push(...tsGate(ROOT, file.texts, values.write));
    // The core's module and code (T6, scripts/texts-core.mjs).
    problems.push(...coreGate(ROOT, file.texts, values.write));
  }
  if (problems.length > 0) {
    for (const problem of problems) console.error(`texts: ${problem}`);
    process.exit(1);
  }
};

if (process.argv[1] === fileURLToPath(import.meta.url)) main();

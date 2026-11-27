// Rich text never reaches the TypeScript SDK's package before its guard is
// complete (NOC-468; NOC-461): the core of crates/nocentraldb-wasm holds
// the rich-text bridge only with the compiler cfg `ncdb_rich_text`, which
// only the project's own tasks set; a core built with it exports a marker,
// the one packages/sdk/richText.mjs names; and the package's build refuses
// a module that holds it.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import * as richText from "../packages/sdk/richText.mjs";

const { RICH_TEXT } = richText;
const refuseRichText = (...args) => richText.refuseRichText(...args);

const ROOT = join(import.meta.dirname, "..");
const read = (path) => readFileSync(join(ROOT, path), "utf8");

test("the package's build refuses a module that holds the marker, and takes one that does not", () => {
  const bytes = (text) => new TextEncoder().encode(text);
  assert.throws(
    () => refuseRichText(bytes(`\0asm…${RICH_TEXT}…`), "core.wasm"),
    /core\.wasm holds rich text, which the package must not ship yet/,
  );
  assert.doesNotThrow(() => refuseRichText(bytes("\0asm…"), "core.wasm"));
});

test("the package's build checks the module it ships", () => {
  assert.match(
    read("packages/sdk/build.mjs"),
    /refuseRichText\(\s*readFileSync\(MODULE\)/,
  );
});

test("the core exports the marker richText.mjs names, and only with the cfg", () => {
  const core = read("crates/nocentraldb-wasm/src/lib.rs");
  const marker = /const RICH_TEXT: &str = "([^"]*)";/;
  assert.match(core, marker);
  assert.equal(core.match(marker)[1], RICH_TEXT);
  assert.match(core, /#\[cfg\(ncdb_rich_text\)\]\s*mod rich_text/);
});

test("only the project's own tasks set the cfg, never a build of the package", () => {
  assert.doesNotMatch(read("packages/sdk/build.mjs"), /ncdb_rich_text/);
  assert.match(
    read("crates/nocentraldb-wasm/moon.yml"),
    /RUSTFLAGS: "--cfg ncdb_rich_text"/,
  );
});

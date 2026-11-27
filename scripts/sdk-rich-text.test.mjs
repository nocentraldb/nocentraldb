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

// Yjs is the app's, shared with the editor's y-tiptap: two copies break
// Yjs's own checks, so the SDK takes it as an optional peer, pinned
// exactly, and needs it only for rich text (Carsten, 2026-11-27, NOC-564).
test("the SDK takes yjs as an optional, exactly pinned peer, never as its own dependency", () => {
  const pkg = JSON.parse(read("packages/sdk/package.json"));
  assert.equal(pkg.dependencies?.yjs, undefined);
  assert.match(pkg.peerDependencies?.yjs ?? "", /^\d+\.\d+\.\d+$/);
  assert.equal(pkg.peerDependenciesMeta?.yjs?.optional, true);
});

// Only an app that opens a rich-text binding needs Yjs: what the SDK's
// entry reaches through static imports imports no `yjs` at run time
// (`import type` is erased), so `nc.open` and everything else load without
// it, also unbundled; `space.richText` loads the binding with a dynamic
// import (provisional, NOC-564).
test("nothing the SDK's entry imports statically imports yjs at run time", () => {
  const seen = new Set();
  const runtime = [];
  const visit = (path) => {
    if (seen.has(path)) return;
    seen.add(path);
    const source = read(path);
    for (const [, specifier] of source.matchAll(
      /^\s*(?:import|export)\s+(?!type\b)[^;]*?\bfrom\s+"([^"]+)";/gms,
    )) {
      if (specifier === "yjs") runtime.push(path);
      if (specifier.startsWith(".")) {
        visit(join(path, "..", specifier).replaceAll("\\", "/"));
      }
    }
  };
  visit("packages/sdk/src/index.ts");
  assert.deepEqual(runtime, []);
});

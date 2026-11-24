// Reads one file's tests for the test-change check (NOC-508).
import { assertions, rustTestRegions } from "./assertions.mjs";
import { mask } from "./mask.mjs";

const lineAt = (text, index) => text.slice(0, index).split("\n").length;

// A Rust function with its attributes in front of it.
const RUST_FN =
  /((?:#\s*\[[^\]]*\]\s*)+)(?:pub(?:\s*\([^)]*\))?\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+([A-Za-z_]\w*)/g;
// The path of one attribute, `#[tokio::test(...)]` → `tokio::test`.
const ATTRIBUTE = /#\s*\[\s*([\w:]+)([^\]]*)\]/g;
const TEST_PATH = /(?:^|::)(?:test|wasm_bindgen_test|rstest|test_case)$/;

// `#[ignore]`, `#[ignore = "why"]` or a `cfg_attr` that adds `ignore`.
const ignores = (attribute) =>
  attribute[1] === "ignore" ||
  (attribute[1] === "cfg_attr" && /\bignore\b/.test(attribute[2]));

function rust(source, kind) {
  const masked = mask(source, kind);
  const text = masked.code;
  const tests = [];
  const ignored = [];
  const bodies = [];
  for (const fn of text.matchAll(RUST_FN)) {
    const attributes = [...fn[1].matchAll(ATTRIBUTE)];
    if (!attributes.some((a) => TEST_PATH.test(a[1]))) continue;
    const end = fn.index + fn[0].length;
    const test = { name: fn[2], line: lineAt(text, end) };
    tests.push(test);
    bodies.push(end);
    if (attributes.some(ignores)) ignored.push(test);
  }
  const regions = kind === "rust" ? null : rustTestRegions(text, bodies);
  return {
    tests,
    ignored,
    markers: [],
    assertions: assertions(masked, kind, regions),
  };
}

// `it(` or `test(`, with modifiers such as `.skip` or `.each(table)`, then
// the name as a string literal, read in the masked code (its inside blank).
// A `.test(` on anything but a test context (`t.test(`) is a method such as
// RegExp's, not a test.
const TS_TEST =
  /(?:(?<![\w$.])|(?<=(?<![\w$.])(?:t|ctx|context)\.))(it|test)((?:\.\w+)*)\s*(?:\([^)]*\)\s*)?\(\s*(["'`])([^"'`]*)\3/g;

// What skips a test or runs only some, each with the key it is counted by:
// a modifier (`it.skip`, `describe.only`, `test.skipIf`), a prefixed call
// (`xit`, `fdescribe`), node:test's options (`skip:`, `only:`, `todo:`) and
// a skip at run time (`t.skip(`, `ctx.skip(`, `this.skip(`).
const TS_MARKERS = [
  [
    /(?<![\w$.])(it|test|describe|suite)\.(skip|only|todo|skipIf|runIf|fails)\b/g,
    (m) => `${m[1]}.${m[2]}`,
  ],
  [/(?<![\w$.])(x(?:it|test|describe)|f(?:it|describe))\s*\(/g, (m) => m[1]],
  [/(?<![\w$.])(skip|only|todo)\s*:/g, (m) => `${m[1]}:`],
  [
    /(?<![\w$.])(t|ctx|context|this)\.(skip|todo)\s*\(/g,
    (m) => `${m[1]}.${m[2]}`,
  ],
];

function typescript(source) {
  const masked = mask(source, "ts");
  const text = masked.code;
  const tests = [...text.matchAll(TS_TEST)].map((m) => {
    const end = m.index + m[0].length - 1;
    return {
      name: masked.plain.slice(end - m[4].length, end),
      line: lineAt(text, m.index),
    };
  });
  const markers = TS_MARKERS.flatMap(([pattern, key]) =>
    [...text.matchAll(pattern)].map((m) => ({
      key: key(m),
      line: lineAt(text, m.index),
    })),
  );
  return { tests, ignored: [], markers, assertions: assertions(masked, "ts") };
}

/**
 * The tests in `text`, a file of kind `kind` (lib/test-changes/files.mjs):
 * `tests` and the `ignored` ones (Rust) by name, and the `markers` that skip
 * tests (TypeScript), each with its line, and the `assertions` in test code
 * (lib/test-changes/assertions.mjs).
 */
export const scan = (text, kind) =>
  kind === "ts" ? typescript(text) : rust(text, kind);

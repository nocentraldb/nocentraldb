// Reads one file's tests for the test-change check (NOC-508).

const lineAt = (text, index) => text.slice(0, index).split("\n").length;

// A Rust function with its attributes in front of it.
const RUST_FN =
  /((?:#\s*\[[^\]]*\]\s*)+)(?:pub(?:\s*\([^)]*\))?\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+([A-Za-z_]\w*)/g;
// The path of one attribute, `#[tokio::test(...)]` → `tokio::test`.
const ATTRIBUTE = /#\s*\[\s*([\w:]+)([^\]]*)\]/g;
const TEST_PATH = /(?:^|::)(?:test|wasm_bindgen_test|rstest|test_case)$/;

function rustTests(text) {
  const tests = [];
  for (const fn of text.matchAll(RUST_FN)) {
    const paths = [...fn[1].matchAll(ATTRIBUTE)].map((a) => a[1]);
    if (!paths.some((path) => TEST_PATH.test(path))) continue;
    tests.push({ name: fn[2], line: lineAt(text, fn.index + fn[0].length) });
  }
  return tests;
}

// `it(` or `test(`, with modifiers such as `.skip` or `.each(table)`, then
// the name as a string literal. A `.test(` on anything but a test context
// (`t.test(`) is a method such as RegExp's, not a test.
const TS_TEST =
  /(?:(?<![\w$.])|(?<=(?<![\w$.])(?:t|ctx|context)\.))(it|test)((?:\.\w+)*)\s*(?:\([^)]*\)\s*)?\(\s*(["'`])((?:\\.|(?!\3)[^\\])*)\3/g;

function tsTests(text) {
  return [...text.matchAll(TS_TEST)].map((m) => ({
    name: m[4],
    line: lineAt(text, m.index),
  }));
}

/** The tests in `text`, a file of kind `kind` (lib/test-changes/files.mjs). */
export function scan(text, kind) {
  return { tests: kind === "ts" ? tsTests(text) : rustTests(text) };
}

// Which files the test-change check reads (NOC-508), and as what.
//
// - "ts": a TypeScript or JavaScript test file: a name with `.test.` or
//   `.spec.` in it, or a file under a directory named test, tests or
//   __tests__. The whole file is test code.
// - "rust": a Rust file under a directory named tests, or named tests.rs,
//   test.rs, *_tests.rs or *_test.rs. The whole file is test code.
// - "rust-src": any other Rust file. Only its tests count: each function
//   with a test attribute, and each item under #[cfg(test)].
// - null: a file the check does not read.

const TS_EXT = /\.(?:ts|tsx|mts|cts|js|jsx|mjs|cjs)$/;
const TEST_DIR = /(?:^|\/)(?:test|tests|__tests__)\//;
const TS_TEST_NAME = /\.(?:test|spec)\.[^/]*$/;
const RUST_TEST_NAME = /(?:^|\/|_)tests?\.rs$/;

export function kindOf(path) {
  if (TS_EXT.test(path)) {
    return TS_TEST_NAME.test(path) || TEST_DIR.test(path) ? "ts" : null;
  }
  if (path.endsWith(".rs")) {
    return /(?:^|\/)tests\//.test(path) || RUST_TEST_NAME.test(path)
      ? "rust"
      : "rust-src";
  }
  return null;
}

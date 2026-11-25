// The value on which a core built with the feature `test-trap` of
// crates/nocentraldb-wasm traps (NOC-499): the browser test of a trap
// (scripts/sdk-browser/trap.e2e.test.mjs) writes it, and the package's
// build (build.mjs) refuses a module that holds it. Not in the package:
// its `files` hold only dist/.

/** The marker, as the core's `TEST_TRAP` holds it. */
export const TEST_TRAP = "nocentraldb:test-trap";

// The value on which a core built with the feature `test-trap` of
// crates/nocentraldb-wasm traps (NOC-499): the browser test of a trap
// (scripts/sdk-browser/trap.e2e.test.mjs) writes it, and the package's
// build (build.mjs) refuses a module that holds it. Not in the package:
// its `files` hold only dist/.

/** The marker, as the core's `TEST_TRAP` holds it. */
export const TEST_TRAP = "nocentraldb:test-trap";

/** Throws when `bytes`, the module at `path`, holds the marker: a core built with the feature `test-trap`, which the package must never ship. */
export const refuseTestTrap = (bytes, path) => {
  if (Buffer.from(bytes).includes(TEST_TRAP)) {
    throw new Error(
      `${path} holds the marker of the feature test-trap: build the package's core without it`,
    );
  }
};

// Ends a test file of the site demo soon after its tests (NOC-341). A
// handle a test leaves open, such as a socket to a browser or a timer,
// would keep the file's process running, and with it the runner, until the
// workflow's time limit. This names what is still open, so the cause shows
// in the log, and ends the process with a failure.
import { after, test as nodeTest } from "node:test";

/** How long a file may take to wind down after its last hook. */
const LINGER_MS = Number(process.env.SITE_DEMO_LINGER_MS ?? 30_000);

/** How long one test of the demo may take before it fails. */
export const TEST_MS = 180_000;

/** A test that fails after TEST_MS rather than waiting without end. */
export const test = (name, fn) => nodeTest(name, { timeout: TEST_MS }, fn);

/**
 * Call once at the top of a test file, before its own after() hooks: the
 * time starts when the file's after() hooks start.
 */
export const endsWhenDone = () => {
  after(() => {
    // Unreferenced: it fires only if something else keeps the process alive.
    setTimeout(() => {
      const open = process
        .getActiveResourcesInfo()
        .filter((name) => name !== "TTYWrap");
      process.stderr.write(
        `${LINGER_MS} ms after its tests, these kept it running: ${open.join(", ")}\n`,
      );
      process.exit(1);
    }, LINGER_MS).unref();
  });
};

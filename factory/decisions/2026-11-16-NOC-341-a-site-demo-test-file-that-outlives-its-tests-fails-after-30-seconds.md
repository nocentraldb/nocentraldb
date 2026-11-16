# 2026-11-16 · NOC-341 · A site-demo test file that outlives its tests fails after 30 seconds, naming what kept it running

- Decided by: coding agent (NOC-341)
- Why: run 43828084330 passed all six tests of `demo.e2e.test.mjs` and then ran into the view job's 30-minute limit. The log cannot tell which handle stayed open, and the hang did not reproduce here in 3 full runs before the change nor in 16 after it. So every way found to stay alive is closed:
  - the server's `close()` waited for a connection a browser left in the middle of a request;
  - `cdp.mjs`'s `close()` waited for an exit event when Chromium had already ended;
  - a call to a Chromium that went away never settled;
  - Chromium could refuse a tab right after launch ("no browser is open"). This was seen once, under the three files' parallel load: the test failed before `close()`, and the Chromium it left kept the file running.
  
  What is left is a guard. Each file arms an unreferenced timer when its `after()` hooks start, and the timer fires only if something keeps the process alive. It then prints the open handles and exits 1, so a leak shows in the log with its cause within half a minute rather than hiding until the time limit. Each test also fails after 3 minutes instead of waiting without end.
- Instead of: exiting 0 when the tests passed, which hides a leak that would grow; `--test-force-exit` or `--test-timeout` in the command, which changes `moon.yml` and the workflow, and gives no hint of the cause; or only raising the job's time limit.
- Topics: testing, tooling, flaky-tests

// A test file that passes and leaves an interval running, for
// teardown.test.mjs: endsWhenDone() ends it and names the interval.
import { test } from "node:test";
import { endsWhenDone } from "../teardown.mjs";

endsWhenDone();

test("passes and leaves a timer", () => {
  setInterval(() => {}, 1000);
});

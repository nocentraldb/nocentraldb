// The mutant runner shared by the workflow contract tests
// (scripts/*-workflow.test.mjs). Every check runs once against the real
// files and must pass, then once against each of its mutants (a clone of the
// files with one property removed or changed) and must reject every one with
// an AssertionError: a check that cannot fail is caught here, on every run.
import assert, { AssertionError } from "node:assert/strict";
import { test } from "node:test";

/**
 * Registers two tests per check: `name`, and `${name}: rejects every mutant`.
 * `files` is the parsed input, `clone(files)` a deep copy a mutant may change.
 */
export function runWorkflowChecks({ files, clone, checks }) {
  for (const { name, check, mutants } of checks) {
    test(name, () => check(files));

    test(`${name}: rejects every mutant`, async (t) => {
      assert.ok(Object.keys(mutants).length > 0, "the check has no mutant");
      for (const [mutant, mutate] of Object.entries(mutants)) {
        await t.test(mutant, () => {
          const ctx = clone(files);
          mutate(ctx);
          assert.throws(() => check(ctx), AssertionError);
        });
      }
    });
  }
}

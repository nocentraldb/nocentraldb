// The model of the browser test that erases a person (NOC-406,
// scripts/sdk-browser/erase.e2e.test.mjs): one personal field, about the
// object itself. `ncdb model build` writes model.lock.json from it;
// `crates/ncdb/tests/fixtures.rs` checks that the lock is still what the
// build writes.
import { nc } from "../../../packages/sdk/src/index.ts";

export const model = nc.model({
  people: nc.collection({ name: nc.string().personal({ person: "self" }) }),
});

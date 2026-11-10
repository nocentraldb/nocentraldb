// The dedicated worker of the browser view test (NOC-174): it loads the
// harness (crates/ncdb-core/examples/browser_view.rs, built by build.mjs)
// and runs each call the page sends, in order.
import init, * as harness from "./pkg/browser_view.js";

const ready = init();
let queue = Promise.resolve();

self.onmessage = ({ data: { id, call, args } }) => {
  queue = queue.then(async () => {
    try {
      await ready;
      self.postMessage({ id, ok: await harness[call](...args) });
    } catch (error) {
      self.postMessage({ id, error: String(error?.message ?? error) });
    }
  });
};

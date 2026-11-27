// The binding's port onto the bridge (NOC-461): `richText.open` leases a
// client id for one field and answers its state; the port then speaks
// `richText.pull`, `richText.write` and `richText.close` under the lease's
// handle (crates/nocentraldb/src/bridge/mod.rs). The worker, the wasm
// module and this code ship in one build, so the answers' shapes are the
// bridge's own.
import type { Opened, RichTextPort } from "./binding.ts";

type Ask = (call: string, args: Record<string, unknown>) => Promise<unknown>;

/** The field a binding opens on. */
export interface RichTextTarget {
  readonly space: string;
  readonly collection: string;
  readonly id: string;
  readonly field: string;
}

/** Leases a client id for `target` and answers what the binding opens with and its port. */
export const openRichText = async (
  ask: Ask,
  target: RichTextTarget,
): Promise<{ opened: Opened; port: RichTextPort }> => {
  const { lease, client, writable, state } = (await ask("richText.open", {
    ...target,
  })) as {
    lease: number;
    client: number;
    writable: boolean;
    state: Uint8Array;
  };
  const port: RichTextPort = {
    pull: async (stateVector) =>
      (await ask("richText.pull", { lease, stateVector })) as Uint8Array,
    write: async (update) => {
      await ask("richText.write", { lease, update });
    },
    close: async () => {
      await ask("richText.close", { lease });
    },
  };
  return { opened: { client, state, writable }, port };
};

// space.status.current().onlyMember (RFC 0009, "Sync and status"; RFC 0010,
// "What it restores"; NOC-494): a space whose only member is this member
// says so, because losing this device loses the space, and the core's
// `spaceStatus` events change it when a second member is added or removed.
// Against a fake core; the core's own tests show when it sends which value
// (crates/nocentraldb/src/status/only_member_tests.rs).
import { expect, expectTypeOf, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { nc } from "../src/index.ts";

const model = nc.model({
  tasks: nc.collection({ title: nc.string() }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };
const SPACE = "0123456789abcdef0123456789abcdef";

const ALONE = {
  role: "admin",
  initialSyncComplete: false,
  behind: 0,
  gaps: [],
  parked: {},
  newestModel: null,
  rejected: {},
  refused: [],
  lostGrants: [],
  equivocations: [],
  exposed: false,
  waitingForKey: false,
  badPersonKeyRecords: [],
  missingPersonKeys: 0,
  provisionalErasures: [],
  onlyMember: true,
};

const fake = (status: Record<string, unknown> = ALONE) => {
  let listener: ((event: Uint8Array) => void) | undefined;
  const core: Core = {
    call: async (bytes) => {
      const { id, call } = decode(bytes) as { id: number; call: string };
      switch (call) {
        case "spaces.create":
          return encode({ id, ok: SPACE });
        case "space.status":
          return encode({ id, ok: status });
        default:
          return encode({ id, ok: null });
      }
    },
    terminate: () => {},
    listen: (given) => {
      listener = given;
    },
  };
  const send = (onlyMember: boolean) =>
    listener?.(
      encode({
        event: "spaceStatus",
        space: SPACE,
        status: { ...ALONE, onlyMember },
      }),
    );
  return { core, send };
};

test("a space this member created alone has it as its only member", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock });
  const project = await db.spaces.create();
  expect(project.status.current().onlyMember).toBe(true);
  expectTypeOf(project.status.current().onlyMember).toEqualTypeOf<boolean>();
  await db.close();
});

test("a second member added, then removed, changes it for subscribers", async () => {
  const { core, send } = fake();
  const db = await connect(core, { model, lock });
  const project = await db.spaces.create();
  const seen: boolean[] = [];
  project.status.subscribe((status) => seen.push(status.onlyMember));
  send(false);
  expect(project.status.current().onlyMember).toBe(false);
  send(true);
  expect(seen).toEqual([true, false, true]);
  await db.close();
});

test("a status without the value reads as not the only member", async () => {
  const { onlyMember: _, ...without } = ALONE;
  const { core } = fake(without);
  const db = await connect(core, { model, lock });
  const project = await db.spaces.create();
  expect(project.status.current().onlyMember).toBe(false);
  await db.close();
});

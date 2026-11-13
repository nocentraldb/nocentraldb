// space.status (RFC 0009, "Sync and status"; NOC-176 slice 4): what a
// space holds that a read does not show. A space handle comes with its
// status, from the core's `space.status`, and the core's `spaceStatus`
// events keep it current (crates/nocentraldb/src/bridge/mod.rs). Against a
// fake core.
import { expect, expectTypeOf, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { nc } from "../src/index.ts";

const model = nc.model({
  tasks: nc.collection({ title: nc.string() }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };
const SPACE = "0123456789abcdef0123456789abcdef";
const OTHER = "fedcba9876543210fedcba9876543210";
const CHANGE = "cc".repeat(32);

const NEW = {
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
  provisionalErasures: [],
};

const fake = (status: Record<string, unknown> = NEW) => {
  let listener: ((event: Uint8Array) => void) | undefined;
  const calls: { call: string; args: Record<string, unknown> }[] = [];
  const core: Core = {
    call: async (bytes) => {
      const { id, call, args } = decode(bytes) as {
        id: number;
        call: string;
        args: Record<string, unknown>;
      };
      calls.push({ call, args });
      switch (call) {
        case "spaces.create":
          return encode({ id, ok: SPACE });
        case "spaces.get":
          return encode({ id, ok: args.space });
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
  const send = (space: string, changed: Record<string, unknown>) =>
    listener?.(
      encode({ event: "spaceStatus", space, status: { ...NEW, ...changed } }),
    );
  return { core, calls, send };
};

test("a space comes with its status, read with space.status.current()", async () => {
  const { core, calls } = fake();
  const db = await connect(core, { model, lock });
  const project = await db.spaces.create();
  expect(project.status.current()).toEqual({
    role: "admin",
    initialSyncComplete: false,
    behind: 0,
    gaps: [],
    parked: {},
    rejected: {},
    refused: [],
    lostGrants: [],
    equivocations: [],
    exposed: false,
    waitingForKey: false,
    badPersonKeyRecords: [],
    provisionalErasures: [],
  });
  expect(calls.map(({ call }) => call)).toEqual([
    "open",
    "spaces.create",
    "space.status",
  ]);
  expect(calls[2]?.args).toEqual({ space: SPACE });
});

test("its values keep their details: reasons, refusals, a newer model, provisional erasures", async () => {
  const erasure = {
    change: CHANGE,
    age: 1500,
    canUndo: true,
    performer: "ab".repeat(32),
    notice: { code: "erasurePending", text: "Personal data was erased." },
  };
  const { core } = fake({
    ...NEW,
    role: "removed",
    parked: { anotherModel: 2 },
    newestModel: 3,
    rejected: { badSignature: 1 },
    refused: [{ change: CHANGE, code: "author_not_member" }],
    provisionalErasures: [erasure],
  });
  const db = await connect(core, { model, lock });
  const status = (await db.spaces.get(SPACE)).status.current();
  expect(status.role).toBe("removed");
  expect(status.parked).toEqual({ anotherModel: 2 });
  expect(status.newestModel).toBe(3);
  expect(status.rejected).toEqual({ badSignature: 1 });
  expect(status.refused).toEqual([
    { change: CHANGE, code: "author_not_member" },
  ]);
  expect(status.provisionalErasures).toEqual([erasure]);
});

test("a spaceStatus event changes current() and calls subscribers of that space only", async () => {
  const { core, send } = fake();
  const db = await connect(core, { model, lock });
  const project = await db.spaces.create();
  const again = await db.spaces.get(SPACE);
  const seen: boolean[] = [];
  const stop = project.status.subscribe((status) =>
    seen.push(status.initialSyncComplete),
  );
  expect(seen).toEqual([false]);
  send(OTHER, { initialSyncComplete: true });
  expect(seen).toEqual([false]);
  send(SPACE, { initialSyncComplete: true, behind: 4 });
  expect(seen).toEqual([false, true]);
  expect(project.status.current().behind).toBe(4);
  expect(again.status.current().behind).toBe(4);
  stop();
  send(SPACE, { behind: 0 });
  expect(seen).toEqual([false, true]);
  await db.close();
});

test("closing the database stops every status subscription", async () => {
  const { core, send } = fake();
  const db = await connect(core, { model, lock });
  const project = await db.spaces.create();
  const seen: number[] = [];
  project.status.subscribe((status) => seen.push(status.behind));
  await db.close();
  send(SPACE, { behind: 9 });
  expect(seen).toEqual([0]);
});

test("its types are RFC 0009's", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock });
  const status = (await db.spaces.create()).status.current();
  expectTypeOf(status.role).toEqualTypeOf<
    "reader" | "writer" | "admin" | "removed"
  >();
  expectTypeOf(status.behind).toEqualTypeOf<number>();
  expectTypeOf(status.gaps).toEqualTypeOf<readonly string[]>();
  expectTypeOf(status.parked).toEqualTypeOf<Readonly<Record<string, number>>>();
  expectTypeOf(status.newestModel).toEqualTypeOf<number | undefined>();
  expectTypeOf(status.refused).toEqualTypeOf<
    readonly { readonly change: string; readonly code: string }[]
  >();
  expectTypeOf(status.provisionalErasures[0]?.age).toEqualTypeOf<
    number | undefined
  >();
});

// Ad-hoc spaces for sub-agents (NOC-318 S4; RFC 0009, "Sub-agents and
// ad-hoc spaces"; the TypeScript shapes the owner decided on NOC-351,
// Carsten, 2026-11-16): `db.spaces.createAdHoc` takes the objects the SDK
// read and answers the space, its notice, each source with its copy and
// the counts; `space.sourceOf` and `space.copyOf` find them later. They
// are the core's `spaces.createAdHoc`, `space.sourceOf` and `space.copyOf`
// (crates/nocentraldb/src/bridge/ad_hoc.rs). Against a fake core.
import { expect, expectTypeOf, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { type Notice, nc, type Space } from "../src/index.ts";

const model = nc.model({
  tasks: nc.collection(
    { title: nc.string(), priority: nc.int() },
    { indexes: { byPriority: nc.index(["priority"]) } },
  ),
  projects: nc.collection({ name: nc.string() }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };

const SPACE = "0123456789abcdef0123456789abcdef";
const AD_HOC = "fedcba9876543210fedcba9876543210";
const TASK = "11".repeat(16);
const PROJECT = "22".repeat(16);
const COPY = "33".repeat(16);
const COPY_2 = "44".repeat(16);

const CREATED = {
  code: "adHocSpaceCreated",
  text: "The ad-hoc space limits what the sub-agent can decrypt.",
};

/** A core that records each call and answers the ad-hoc calls. */
const fake = () => {
  const calls: { call: string; args: Record<string, unknown> }[] = [];
  const core: Core = {
    call: async (bytes) => {
      const { id, call, args } = decode(bytes) as {
        id: number;
        call: string;
        args: Record<string, unknown>;
      };
      calls.push({ call, args });
      const object = (collection: string, $id: string) => ({
        $id,
        $space: SPACE,
        $deleted: false,
        fields: collection === "tasks" ? { title: "A" } : { name: "P" },
      });
      switch (call) {
        case "get":
          return encode({
            id,
            ok: object(args.collection as string, args.id as string),
          });
        case "find":
          return encode({
            id,
            ok: {
              items: [object("tasks", TASK)],
              cursor: null,
              hasMore: false,
            },
          });
        case "spaces.get":
          return encode({ id, ok: args.space });
        case "space.status":
          return encode({ id, ok: { role: "admin" } });
        case "spaces.createAdHoc":
          return encode({
            id,
            ok: {
              space: AD_HOC,
              notice: CREATED,
              copies: [
                { source: TASK, copy: COPY },
                { source: PROJECT, copy: COPY_2 },
              ],
              clearedLinks: 1,
              pluginFieldsNotCopied: 0,
            },
          });
        case "space.sourceOf":
          return encode({
            id,
            ok: args.copy === COPY ? { space: SPACE, id: TASK } : null,
          });
        case "space.copyOf":
          return encode({ id, ok: args.source === TASK ? COPY : null });
        default:
          return encode({ id, ok: null });
      }
    },
    terminate: () => {},
    listen: () => {},
  };
  return { core, calls };
};

const asked = (calls: ReturnType<typeof fake>["calls"], call: string) =>
  calls.filter((c) => c.call === call).map((c) => c.args);

test("createAdHoc sends each object the SDK read with its space, collection and id", async () => {
  const { core, calls } = fake();
  const db = await connect(core, { model, lock });
  const space = await db.spaces.get(SPACE);
  const project = await space.get(model.projects, PROJECT);
  const {
    items: [task],
  } = await space.find(model.tasks.indexes.byPriority);

  await db.spaces.createAdHoc({
    copy: [task, project].filter((o) => o !== undefined),
  });

  expect(asked(calls, "spaces.createAdHoc")).toEqual([
    {
      copy: [
        { space: SPACE, collection: "tasks", id: TASK },
        { space: SPACE, collection: "projects", id: PROJECT },
      ],
    },
  ]);
});

test("createAdHoc gives the space, its notice, each source with its copy and the counts", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock });
  const space = await db.spaces.get(SPACE);
  const task = await space.get(model.tasks, TASK);
  if (task === undefined) throw new Error("read");

  const created = await db.spaces.createAdHoc({ copy: [task] });

  expect(created.space.id).toBe(AD_HOC);
  expect(created.notice).toEqual(CREATED);
  expect(created.copies).toEqual([
    { source: TASK, copy: COPY },
    { source: PROJECT, copy: COPY_2 },
  ]);
  expect(created.clearedLinks).toBe(1);
  expect(created.pluginFieldsNotCopied).toBe(0);
});

test("createAdHoc passes personal and local only when given", async () => {
  const { core, calls } = fake();
  const db = await connect(core, { model, lock });
  const space = await db.spaces.get(SPACE);
  const task = await space.get(model.tasks, TASK);
  if (task === undefined) throw new Error("read");

  await db.spaces.createAdHoc({ copy: [task], personal: false, local: true });

  expect(asked(calls, "spaces.createAdHoc")[0]).toEqual({
    copy: [{ space: SPACE, collection: "tasks", id: TASK }],
    personal: false,
    local: true,
  });
});

test("an object the SDK did not read is refused with invalidValue, before the core", async () => {
  const { core, calls } = fake();
  const db = await connect(core, { model, lock });
  const space = await db.spaces.get(SPACE);
  const task = await space.get(model.tasks, TASK);
  if (task === undefined) throw new Error("read");

  // A spread is a new object: the SDK no longer knows its collection.
  await expect(
    db.spaces.createAdHoc({ copy: [{ ...task }] }),
  ).rejects.toMatchObject({ code: "invalidValue" });
  expect(asked(calls, "spaces.createAdHoc")).toEqual([]);
});

test("an object another database read is refused with invalidValue", async () => {
  const one = fake();
  const other = fake();
  const db = await connect(one.core, { model, lock });
  const otherDb = await connect(other.core, { model, lock });
  const task = await (await otherDb.spaces.get(SPACE)).get(model.tasks, TASK);
  if (task === undefined) throw new Error("read");

  await expect(db.spaces.createAdHoc({ copy: [task] })).rejects.toMatchObject({
    code: "invalidValue",
  });
  expect(asked(one.calls, "spaces.createAdHoc")).toEqual([]);
});

test("space.sourceOf and space.copyOf ask the core, and give undefined for none", async () => {
  const { core, calls } = fake();
  const db = await connect(core, { model, lock });
  const adHoc = await db.spaces.get(AD_HOC);

  expect(await adHoc.sourceOf(COPY)).toEqual({ space: SPACE, id: TASK });
  expect(await adHoc.sourceOf(TASK)).toBeUndefined();
  expect(await adHoc.copyOf(SPACE, TASK)).toBe(COPY);
  expect(await adHoc.copyOf(SPACE, PROJECT)).toBeUndefined();

  expect(asked(calls, "space.sourceOf")[0]).toEqual({
    space: AD_HOC,
    copy: COPY,
  });
  expect(asked(calls, "space.copyOf")[0]).toEqual({
    space: AD_HOC,
    sourceSpace: SPACE,
    source: TASK,
  });
});

test("their types are the owner's (NOC-351)", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock });
  const created = await db.spaces.createAdHoc({ copy: [] });
  expectTypeOf(created.space).toEqualTypeOf<Space>();
  expectTypeOf(created.notice).toEqualTypeOf<Notice>();
  expectTypeOf(created.copies).toEqualTypeOf<
    readonly { readonly source: string; readonly copy: string }[]
  >();
  expectTypeOf(created.clearedLinks).toEqualTypeOf<number>();
  expectTypeOf(created.pluginFieldsNotCopied).toEqualTypeOf<number>();
  expectTypeOf(created.space.sourceOf).returns.toEqualTypeOf<
    Promise<{ readonly space: string; readonly id: string } | undefined>
  >();
  expectTypeOf(created.space.copyOf).returns.toEqualTypeOf<
    Promise<string | undefined>
  >();
});

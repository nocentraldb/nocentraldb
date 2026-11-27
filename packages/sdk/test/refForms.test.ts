// The forms an nc.ref field takes and reads in the TypeScript SDK (RFC
// 0018, "One function, two cases" and "Existing link fields read as
// references"), against a fake core that answers the worker bridge's
// protocol. A reference field takes `{ space, id }`, an object the SDK
// read, or a bare object id, which names the written object's own space;
// each travels to the core as `{ space, id }`. A field written `nc.ref`
// that the lock keeps as `ncdb/link` comes back from the core as an object
// id and reads as `{ space, id }` with the object's own space.
import { describe, expect, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { nc } from "../src/index.ts";

const model = nc.model({
  tasks: nc.collection({ title: nc.string(), project: nc.ref("projects") }),
  projects: nc.collection({ name: nc.string() }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };

const SPACE = "0123456789abcdef0123456789abcdef";
const OTHER = "00000000000000000000000000000001";
const OBJECT = "fedcba9876543210fedcba9876543210";
const TARGET = "11111111111111111111111111111111";

type Request = { id: number; call: string; args: Record<string, unknown> };

/** A core with a space, which answers a get with `fields`. */
const core = (fields: Record<string, unknown> = {}) => {
  const requests: Request[] = [];
  const answer = (request: Request): unknown => {
    switch (request.call) {
      case "spaces.create":
        return SPACE;
      case "write":
        return { change: "aa".repeat(32), ids: [OBJECT] };
      case "get":
        return { $id: OBJECT, $space: SPACE, $deleted: false, fields };
      default:
        return null;
    }
  };
  const fake: Core = {
    call: async (bytes) => {
      const request = decode(bytes) as Request;
      requests.push(request);
      return encode({ id: request.id, ok: answer(request) });
    },
    terminate: () => {},
    listen: () => {},
  };
  return { fake, requests };
};

const written = (requests: Request[]) => {
  const write = requests.find((r) => r.call === "write");
  const ops = (write?.args.ops ?? []) as { values: Record<string, unknown> }[];
  return ops[0]?.values;
};

describe("writing a reference", () => {
  test("{ space, id } travels as it is", async () => {
    const { fake, requests } = core();
    const space = await (await connect(fake, { model, lock })).spaces.create();
    await space.create(model.tasks, { project: { space: OTHER, id: TARGET } });
    expect(written(requests)).toEqual({
      project: { space: OTHER, id: TARGET },
    });
  });

  test("a bare object id names the written object's own space", async () => {
    const { fake, requests } = core();
    const space = await (await connect(fake, { model, lock })).spaces.create();
    await space.update(model.tasks, OBJECT, { project: TARGET });
    expect(written(requests)).toEqual({
      project: { space: SPACE, id: TARGET },
    });
  });

  test("an object the SDK read names its $space and $id, and nothing else of it", async () => {
    const { fake, requests } = core();
    const space = await (await connect(fake, { model, lock })).spaces.create();
    const project = {
      $id: TARGET,
      $space: OTHER,
      $deleted: false,
      name: "Ship",
    };
    await space.create(model.tasks, { project });
    expect(written(requests)).toEqual({
      project: { space: OTHER, id: TARGET },
    });
  });

  test("null still clears it", async () => {
    const { fake, requests } = core();
    const space = await (await connect(fake, { model, lock })).spaces.create();
    await space.update(model.tasks, OBJECT, { project: null });
    expect(written(requests)).toEqual({ project: null });
  });

  test("any other value is no reference, for the type checker", async () => {
    const { fake } = core();
    const space = await (await connect(fake, { model, lock })).spaces.create();
    // @ts-expect-error: a number is no reference.
    await space.create(model.tasks, { project: 7 }).catch(() => undefined);
  });
});

describe("reading a field written nc.ref that the lock keeps as ncdb/link", () => {
  test("an object id reads as { space, id } with the object's own space", async () => {
    const { fake } = core({ title: "Ship", project: TARGET });
    const space = await (await connect(fake, { model, lock })).spaces.create();
    const task = await space.get(model.tasks, OBJECT);
    expect(task?.project).toEqual({ space: SPACE, id: TARGET });
  });

  test("a reference the core reads as { space, id } stays as it is", async () => {
    const { fake } = core({ project: { space: OTHER, id: TARGET } });
    const space = await (await connect(fake, { model, lock })).spaces.create();
    const task = await space.get(model.tasks, OBJECT);
    expect(task?.project).toEqual({ space: OTHER, id: TARGET });
  });
});

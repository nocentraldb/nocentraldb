// A day and a point in time in the TypeScript SDK (RFC 0005, "Days and
// points in time"; Carsten, 2026-11-21, NOC-434), against a fake core that
// answers the worker bridge's protocol: a day is written and read as its
// "YYYY-MM-DD" string, a point in time as a Date, which travels to the core
// as its milliseconds. Anything else, or a value outside the years 1 to
// 9999, is refused with invalidValue before the core.
import { describe, expect, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { nc } from "../src/index.ts";

const model = nc.model({
  events: nc.collection(
    { doneOn: nc.date(), occurredAt: nc.timestamp() },
    {
      indexes: {
        byOccurredAt: nc.index(["occurredAt"], { equal: 1 }),
      },
    },
  ),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };

const SPACE = "0123456789abcdef0123456789abcdef";
const OBJECT = "fedcba9876543210fedcba9876543210";
/** 2026-11-21T19:08:44.636Z. */
const NOW = 1_795_288_124_636;

type Request = { id: number; call: string; args: Record<string, unknown> };

/** A core with a space, which answers a get or a find with `fields`. */
const core = (fields: Record<string, unknown> = {}) => {
  const requests: Request[] = [];
  const read = { $id: OBJECT, $space: SPACE, $deleted: false, fields };
  const answer = (request: Request): unknown => {
    switch (request.call) {
      case "spaces.create":
        return SPACE;
      case "write":
        return { change: "aa".repeat(32), ids: [OBJECT] };
      case "get":
        return read;
      case "find":
        return { items: [read], cursor: null, hasMore: false };
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

describe("writing a day and a point in time", () => {
  test("a day travels as its text, a point in time as its milliseconds", async () => {
    const { fake, requests } = core();
    const db = await connect(fake, { model, lock });
    const space = await db.spaces.create();
    await space.create(model.events, {
      doneOn: "1969-12-31",
      occurredAt: new Date(NOW),
    });
    expect(written(requests)).toEqual({
      doneOn: "1969-12-31",
      occurredAt: NOW,
    });
  });

  test("null clears either", async () => {
    const { fake, requests } = core();
    const db = await connect(fake, { model, lock });
    const space = await db.spaces.create();
    await space.update(model.events, OBJECT, {
      doneOn: null as never,
      occurredAt: null as never,
    });
    expect(written(requests)).toEqual({ doneOn: null, occurredAt: null });
  });

  test("anything but a day's text, or a valid Date, is refused before the core", async () => {
    const { fake, requests } = core();
    const db = await connect(fake, { model, lock });
    const space = await db.spaces.create();
    const wrong: Record<string, unknown>[] = [
      { doneOn: new Date(NOW) },
      { doneOn: "2026-02-30" },
      { doneOn: "21.11.2026" },
      { doneOn: "0000-12-31" },
      { doneOn: "2026-11-21T00:00:00Z" },
      { doneOn: 20778 },
      { occurredAt: "2026-11-21T19:08:44.636Z" },
      { occurredAt: NOW },
      { occurredAt: new Date("not a time") },
      { occurredAt: new Date(253_402_300_800_000) },
      { occurredAt: new Date(-62_135_596_800_001) },
    ];
    for (const values of wrong) {
      const refused = await space
        .create(model.events, values as never)
        .catch((e) => e);
      expect(refused.code, JSON.stringify(values)).toBe("invalidValue");
    }
    expect(requests.map((r) => r.call)).not.toContain("write");
  });

  test("the ends of the range are written", async () => {
    const { fake, requests } = core();
    const db = await connect(fake, { model, lock });
    const space = await db.spaces.create();
    await space.create(model.events, {
      doneOn: "0001-01-01",
      occurredAt: new Date(253_402_300_799_999),
    });
    await space.create(model.events, {
      doneOn: "9999-12-31",
      occurredAt: new Date(-62_135_596_800_000),
    });
    expect(requests.filter((r) => r.call === "write")).toHaveLength(2);
  });
});

describe("reading a day and a point in time", () => {
  test("space.get gives a day as its text and a point in time as a Date", async () => {
    const { fake } = core({ doneOn: "2026-11-21", occurredAt: NOW });
    const db = await connect(fake, { model, lock });
    const space = await db.spaces.create();
    const event = await space.get(model.events, OBJECT);
    expect(event?.doneOn).toBe("2026-11-21");
    expect(event?.occurredAt).toBeInstanceOf(Date);
    expect(event?.occurredAt?.getTime()).toBe(NOW);
    expect(event?.occurredAt?.toISOString()).toBe("2026-11-21T19:08:44.636Z");
  });

  test("a find gives a point in time as a Date, and its equality takes a Date", async () => {
    const { fake, requests } = core({ occurredAt: -1 });
    const db = await connect(fake, { model, lock });
    const page = await db.find(
      model.events.indexes.byOccurredAt.equal(new Date(-1)),
    );
    expect(page.items[0]?.occurredAt?.toISOString()).toBe(
      "1969-12-31T23:59:59.999Z",
    );
    const find = requests.find((r) => r.call === "find");
    expect(find?.args.query).toMatchObject({ equal: [-1] });
  });
});

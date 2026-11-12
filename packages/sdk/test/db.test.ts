// The TypeScript API of slice 1 of NOC-176 (RFC 0009, "Open and close",
// "One write, one change", "Get by id", "Spaces", "Errors"), against a
// fake core that answers the worker bridge's protocol
// (crates/nocentraldb/src/bridge/mod.rs): each test checks what the SDK
// asks the core and what it makes of the answer. Playwright's Chromium
// runs the same calls against the real worker (test/browser/).
import { describe, expect, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/db.ts";
import { nc } from "../src/index.ts";

const model = nc.model({
  tasks: nc.collection({
    title: nc.string(),
    priority: nc.int(),
    score: nc.float(),
    tags: nc.set(nc.string()),
  }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };

const SPACE = "0123456789abcdef0123456789abcdef";
const OBJECT = "fedcba9876543210fedcba9876543210";
const CHANGE = "aa".repeat(32);

type Request = { id: number; call: string; args: Record<string, unknown> };
type Answer = { ok: unknown } | { error: Record<string, unknown> };

/** A core that records each request and answers with `answer`. */
const fake = (answer: (request: Request) => Answer = () => ({ ok: null })) => {
  const requests: Request[] = [];
  let terminated = false;
  const core: Core = {
    call: async (bytes) => {
      const request = decode(bytes) as Request;
      requests.push(request);
      return encode({ id: request.id, ...answer(request) });
    },
    terminate: () => {
      terminated = true;
    },
  };
  return { core, requests, terminated: () => terminated };
};

const calls = (requests: Request[]) => requests.map((r) => r.call);

/** A core with a space and answers for writes and gets. */
const withSpace = (
  more: (request: Request) => Answer | undefined = () => undefined,
) =>
  fake((request) => {
    const answer = more(request);
    if (answer) return answer;
    switch (request.call) {
      case "spaces.create":
        return { ok: SPACE };
      case "spaces.get":
        return { ok: request.args.space };
      case "write":
        return { ok: { change: CHANGE, ids: [] } };
      default:
        return { ok: null };
    }
  });

describe("nc.open", () => {
  test("opens the database named default when the app names none", async () => {
    const { core, requests } = fake();
    await connect(core, { model, lock });
    expect(requests).toEqual([
      {
        id: 1,
        call: "open",
        args: { lock: JSON.stringify(lock), name: "default" },
      },
    ]);
  });

  test("passes the name and the relay", async () => {
    const { core, requests } = fake();
    await connect(core, {
      model,
      lock,
      name: "work_2",
      relay: "wss://relay.example.com",
    });
    expect(requests[0]?.args).toEqual({
      lock: JSON.stringify(lock),
      name: "work_2",
      relay: "wss://relay.example.com",
    });
  });

  test.each(["", "Work", "a b", "ü", "x".repeat(65), "a/b"])(
    "refuses the name %j with invalidValue and starts no core",
    async (name) => {
      const { core, requests, terminated } = fake();
      const refused = await connect(core, { model, lock, name }).catch(
        (e) => e,
      );
      expect(refused).toBeInstanceOf(nc.Error);
      expect(refused.code).toBe("invalidValue");
      expect(requests).toEqual([]);
      expect(terminated()).toBe(true);
    },
  );

  test("accepts a name of 64 characters", async () => {
    const { core } = fake();
    await expect(
      connect(core, { model, lock, name: "a".repeat(64) }),
    ).resolves.toBeDefined();
  });

  test("rejects with the core's error as nc.Error and stops the core", async () => {
    const { core, terminated } = fake(() => ({
      error: { code: "modelRefused", message: "the lock is not additive" },
    }));
    const refused = await connect(core, { model, lock }).catch((e) => e);
    expect(refused).toBeInstanceOf(nc.Error);
    expect(refused).toBeInstanceOf(Error);
    expect(refused.code).toBe("modelRefused");
    expect(refused.message).toBe("the lock is not additive");
    expect(refused.ids).toEqual([]);
    expect(terminated()).toBe(true);
  });
});

describe("nc.Error", () => {
  test("carries the code, the ids as text and the fingerprint", async () => {
    const fingerprint = "12345 ".repeat(12).trim();
    const { core } = withSpace((request) =>
      request.call === "write"
        ? {
            error: {
              code: "unknownAccount",
              message: "confirm the fingerprint",
              ids: [OBJECT],
              fingerprint,
            },
          }
        : undefined,
    );
    const db = await connect(core, { model, lock });
    const project = await db.spaces.create();
    const refused = await project
      .write((tx) => {
        tx.delete(model.tasks, OBJECT);
      })
      .catch((e) => e);
    expect(refused).toBeInstanceOf(nc.Error);
    expect(refused.code).toBe("unknownAccount");
    expect(refused.ids).toEqual([OBJECT]);
    expect(refused.fingerprint).toBe(fingerprint);
    expect(refused.name).toBe("nc.Error");
  });

  test("has no fingerprint unless the core gives one", () => {
    const error = new nc.Error("notFound", "no such object", [OBJECT]);
    expect(error.fingerprint).toBeUndefined();
    expect(error.ids).toEqual([OBJECT]);
  });
});

describe("spaces", () => {
  test("create, get and list ask the core and give spaces with their id", async () => {
    const { core, requests } = withSpace((request) =>
      request.call === "spaces.list"
        ? { ok: [{ id: SPACE, role: "admin" }] }
        : undefined,
    );
    const db = await connect(core, { model, lock });
    const created = await db.spaces.create();
    const same = await db.spaces.get(SPACE);
    const all = await db.spaces.list();
    expect(created.id).toBe(SPACE);
    expect(same.id).toBe(SPACE);
    expect(all).toEqual([{ id: SPACE, role: "admin" }]);
    expect(calls(requests)).toEqual([
      "open",
      "spaces.create",
      "spaces.get",
      "spaces.list",
    ]);
    expect(requests[2]?.args).toEqual({ space: SPACE });
  });

  test("get of a space the copy does not hold rejects with notFound", async () => {
    const { core } = withSpace((request) =>
      request.call === "spaces.get"
        ? {
            error: { code: "notFound", message: "no such space", ids: [SPACE] },
          }
        : undefined,
    );
    const db = await connect(core, { model, lock });
    const refused = await db.spaces.get(SPACE).catch((e) => e);
    expect(refused.code).toBe("notFound");
    expect(refused.ids).toEqual([SPACE]);
  });
});

describe("space.write", () => {
  test("sends one write with every call, collections by name", async () => {
    const { core, requests } = withSpace();
    const db = await connect(core, { model, lock });
    const project = await db.spaces.create();
    let created = "";
    await project.write((tx) => {
      created = tx.create(model.tasks, {
        title: "Ship",
        priority: 2,
        score: undefined,
      });
      tx.update(model.tasks, OBJECT, {
        tags: nc.add("docs", "sdk"),
        title: null,
      });
      tx.update(model.tasks, OBJECT, { tags: nc.remove("old") });
      tx.delete(model.tasks, OBJECT);
      tx.restore(model.tasks, OBJECT);
    });
    expect(created).toMatch(/^[0-9a-f]{32}$/);
    const write = requests.find((r) => r.call === "write");
    expect(write?.args).toEqual({
      space: SPACE,
      ops: [
        {
          op: "create",
          collection: "tasks",
          id: created,
          values: { title: "Ship", priority: 2 },
        },
        {
          op: "update",
          collection: "tasks",
          id: OBJECT,
          values: { tags: { $add: ["docs", "sdk"] }, title: null },
        },
        {
          op: "update",
          collection: "tasks",
          id: OBJECT,
          values: { tags: { $remove: ["old"] } },
        },
        { op: "delete", collection: "tasks", id: OBJECT },
        { op: "restore", collection: "tasks", id: OBJECT },
      ],
    });
  });

  test("draws a new id per create, 128 random bits as 32 hex digits", async () => {
    const { core } = withSpace();
    const db = await connect(core, { model, lock });
    const project = await db.spaces.create();
    const ids: string[] = [];
    await project.write((tx) => {
      ids.push(tx.create(model.tasks, {}), tx.create(model.tasks, {}));
    });
    expect(new Set(ids).size).toBe(2);
  });

  test("resolves with the change and the created ids the core answers", async () => {
    const { core } = withSpace((request) =>
      request.call === "write"
        ? {
            ok: {
              change: CHANGE,
              ids: (request.args.ops as { id: string }[]).map((o) => o.id),
            },
          }
        : undefined,
    );
    const db = await connect(core, { model, lock });
    const project = await db.spaces.create();
    let id = "";
    const result = await project.write((tx) => {
      id = tx.create(model.tasks, { title: "a" });
    });
    expect(result).toEqual({ change: CHANGE, ids: [id] });
  });

  test("the shorthands are one write each", async () => {
    const { core, requests } = withSpace();
    const db = await connect(core, { model, lock });
    const project = await db.spaces.create();
    await project.create(model.tasks, { title: "a" });
    await project.update(model.tasks, OBJECT, { title: "b" });
    await project.delete(model.tasks, OBJECT);
    await project.restore(model.tasks, OBJECT);
    const writes = requests.filter((r) => r.call === "write");
    expect(
      writes.map((w) => (w.args.ops as { op: string }[]).map((o) => o.op)),
    ).toEqual([["create"], ["update"], ["delete"], ["restore"]]);
  });

  test("refuses an int that is not a safe integer with invalidValue, before the core", async () => {
    const { core, requests } = withSpace();
    const db = await connect(core, { model, lock });
    const project = await db.spaces.create();
    for (const priority of [2 ** 53, 1.5, Number.NaN]) {
      const refused = await project
        .create(model.tasks, { priority })
        .catch((e) => e);
      expect(refused.code).toBe("invalidValue");
    }
    expect(calls(requests)).not.toContain("write");
  });

  test("normalises text to NFC", async () => {
    const { core, requests } = withSpace();
    const db = await connect(core, { model, lock });
    const project = await db.spaces.create();
    await project.create(model.tasks, { title: "é" });
    const write = requests.find((r) => r.call === "write");
    const op = ((write?.args.ops ?? []) as { values: { title: string } }[])[0];
    expect(op?.values.title).toBe("é");
  });

  test("a function that throws writes nothing", async () => {
    const { core, requests } = withSpace();
    const db = await connect(core, { model, lock });
    const project = await db.spaces.create();
    await expect(
      project.write(() => {
        throw new Error("app bug");
      }),
    ).rejects.toThrow("app bug");
    expect(calls(requests)).not.toContain("write");
  });
});

describe("space.get", () => {
  test("gives the object with $id, $space, $deleted and its fields", async () => {
    const { core, requests } = withSpace((request) =>
      request.call === "get"
        ? {
            ok: {
              $id: OBJECT,
              $space: SPACE,
              $deleted: false,
              fields: { title: "Ship", tags: ["a"] },
            },
          }
        : undefined,
    );
    const db = await connect(core, { model, lock });
    const project = await db.spaces.create();
    const task = await project.get(model.tasks, OBJECT);
    expect(task).toEqual({
      $id: OBJECT,
      $space: SPACE,
      $deleted: false,
      title: "Ship",
      tags: ["a"],
    });
    expect(requests.at(-1)?.args).toEqual({
      space: SPACE,
      collection: "tasks",
      id: OBJECT,
    });
  });

  test("gives undefined when the copy holds no such object", async () => {
    const { core } = withSpace();
    const db = await connect(core, { model, lock });
    const project = await db.spaces.create();
    expect(await project.get(model.tasks, OBJECT)).toBeUndefined();
  });
});

describe("db.close", () => {
  test("closes the core, and every later call fails with closed", async () => {
    const { core, requests, terminated } = withSpace();
    const db = await connect(core, { model, lock });
    const project = await db.spaces.create();
    await db.close();
    expect(calls(requests).at(-1)).toBe("close");
    expect(terminated()).toBe(true);
    for (const call of [
      () => db.spaces.create(),
      () => db.spaces.list(),
      () => project.get(model.tasks, OBJECT),
      () => project.create(model.tasks, {}),
    ]) {
      const refused = await call().catch((e) => e);
      expect(refused).toBeInstanceOf(nc.Error);
      expect(refused.code).toBe("closed");
    }
    expect(calls(requests).at(-1)).toBe("close");
  });
});

test("a collection that is not in the model is refused with notFound", async () => {
  const { core } = withSpace();
  const db = await connect(core, { model, lock });
  const project = await db.spaces.create();
  const other = nc.collection({ title: nc.string() });
  const refused = await project.get(other, OBJECT).catch((e) => e);
  expect(refused.code).toBe("notFound");
});

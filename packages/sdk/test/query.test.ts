// Queries and subscriptions (RFC 0009, "Queries and indexes", "Subscribe";
// NOC-176 slice 2): every declared index has a handle on its collection,
// `.equal` is checked against the index's equality prefix, and `db.find`,
// `space.find` and the subscriptions ask the core and re-run on its fold
// events. The calls run against a fake core that answers the bridge's
// protocol (crates/nocentraldb/src/bridge/mod.rs).
import { describe, expect, expectTypeOf, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { nc, type Page, type Query } from "../src/index.ts";

const model = nc.model({
  tasks: nc.collection(
    {
      title: nc.string(),
      status: nc.oneOf(["open", "done"]),
      priority: nc.int(),
    },
    {
      indexes: {
        byStatusPriority: nc.index(["status", "priority"], { equal: 1 }),
        byPriority: nc.index(["priority"]),
        open: nc.index(["status", "priority"], { equal: 1, scope: "space" }),
      },
    },
  ),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };

const SPACE = "0123456789abcdef0123456789abcdef";
const OTHER = "fedcba9876543210fedcba9876543210";
const TASK = "00".repeat(16);

type Request = { id: number; call: string; args: Record<string, unknown> };
type Answer = { ok: unknown } | { error: Record<string, unknown> };

/** A core that records each request, answers with `answer`, and can send fold events. */
const fake = (
  answer: (request: Request) => Answer | undefined = () => undefined,
) => {
  const requests: Request[] = [];
  let listener: ((event: Uint8Array) => void) | undefined;
  const core: Core = {
    call: async (bytes) => {
      const request = decode(bytes) as Request;
      requests.push(request);
      const given = answer(request) ?? {
        ok:
          request.call === "spaces.create" || request.call === "spaces.get"
            ? SPACE
            : request.call === "find"
              ? { items: [], cursor: null, hasMore: false }
              : null,
      };
      return encode({ id: request.id, ...given });
    },
    terminate: () => {},
    listen: (given) => {
      listener = given;
    },
  };
  const fold = (...folds: { space: string; collections: string[] }[]) =>
    listener?.(encode({ event: "folded", folds }));
  return { core, requests, fold };
};

const finds = (requests: Request[]) =>
  requests.filter((r) => r.call === "find");

/** Lets the SDK run what an event scheduled. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("index handles and their types", () => {
  test("every declared index has a handle on its collection", () => {
    expect(model.tasks.indexes.byStatusPriority).toBeDefined();
    expect(model.tasks.indexes.byPriority).toBeDefined();
  });

  test("equal takes the index's equality prefix, in count and type", () => {
    model.tasks.indexes.byStatusPriority.equal("open");
    // @ts-expect-error: the index fixes one field
    model.tasks.indexes.byStatusPriority.equal("open", 2);
    // @ts-expect-error: status is a choice of "open" or "done"
    model.tasks.indexes.byStatusPriority.equal(3);
    // @ts-expect-error: byPriority fixes no field
    model.tasks.indexes.byPriority.equal(1);
  });

  test("a query is a Query of the collection's objects, and db.find gives its Page", () => {
    type Task = nc.Infer<typeof model.tasks>;
    const query = model.tasks.indexes.byPriority.take(5);
    expectTypeOf(query).toMatchTypeOf<Query<Task>>();
    type Found = Awaited<
      ReturnType<Awaited<ReturnType<typeof connect>>["find"]>
    >;
    expectTypeOf<Found>().toMatchTypeOf<Page<unknown>>();
  });

  test("a query is immutable: take and equal return new queries", () => {
    const all = model.tasks.indexes.byPriority;
    const five = all.take(5);
    expect(five).not.toBe(all);
  });
});

describe("db.find and space.find", () => {
  test("db.find asks the core for the page of the index", async () => {
    const { core, requests } = fake();
    const db = await connect(core, { model, lock });
    await db.find(model.tasks.indexes.byStatusPriority.equal("open").take(20));
    await db.find(model.tasks.indexes.byPriority);
    expect(finds(requests).map((r) => r.args)).toEqual([
      {
        query: {
          collection: "tasks",
          index: "byStatusPriority",
          equal: ["open"],
          take: 20,
        },
      },
      { query: { collection: "tasks", index: "byPriority" } },
    ]);
  });

  test("a page gives its items as objects, its cursor redacted, and hasMore", async () => {
    const { core } = fake((r) =>
      r.call === "find"
        ? {
            ok: {
              items: [
                {
                  $id: TASK,
                  $space: SPACE,
                  $deleted: false,
                  fields: { title: "a", priority: 1 },
                },
              ],
              cursor: 7,
              hasMore: true,
            },
          }
        : undefined,
    );
    const db = await connect(core, { model, lock });
    const page = await db.find(model.tasks.indexes.byPriority.take(1));
    expect(page.items).toEqual([
      { $id: TASK, $space: SPACE, $deleted: false, title: "a", priority: 1 },
    ]);
    expect(page.hasMore).toBe(true);
    expect(String(page.cursor)).toBe("[redacted]");
    expect(JSON.stringify({ cursor: page.cursor })).toBe(
      '{"cursor":"[redacted]"}',
    );
  });

  test("after continues from a page's cursor", async () => {
    const { core, requests } = fake((r) =>
      r.call === "find"
        ? { ok: { items: [], cursor: 7, hasMore: true } }
        : undefined,
    );
    const db = await connect(core, { model, lock });
    const query = model.tasks.indexes.byPriority.take(1);
    const page = await db.find(query);
    if (page.cursor === undefined) throw new Error("a cursor");
    await db.find(query.after(page.cursor));
    expect(finds(requests)[1]?.args).toEqual({
      query: { collection: "tasks", index: "byPriority", take: 1, after: 7 },
    });
  });

  test("a page without more items has no cursor", async () => {
    const { core } = fake();
    const db = await connect(core, { model, lock });
    const page = await db.find(model.tasks.indexes.byPriority);
    expect(page.cursor).toBeUndefined();
    expect(page.hasMore).toBe(false);
  });

  test("a cursor of another database is refused with cursorOfAnotherModel, before the core", async () => {
    const first = fake((r) =>
      r.call === "find"
        ? { ok: { items: [], cursor: 1, hasMore: true } }
        : undefined,
    );
    const one = await connect(first.core, { model, lock });
    const page = await one.find(model.tasks.indexes.byPriority);
    const second = fake();
    const two = await connect(second.core, { model, lock });
    if (page.cursor === undefined) throw new Error("a cursor");
    const refused = await two
      .find(model.tasks.indexes.byPriority.after(page.cursor))
      .catch((e) => e);
    expect(refused).toBeInstanceOf(nc.Error);
    expect(refused.code).toBe("cursorOfAnotherModel");
    expect(finds(second.requests)).toEqual([]);
  });

  test("space.find asks for the page in its space", async () => {
    const { core, requests } = fake();
    const db = await connect(core, { model, lock });
    const project = await db.spaces.create();
    await project.find(model.tasks.indexes.open.equal("open"));
    expect(finds(requests)[0]?.args).toEqual({
      space: SPACE,
      query: { collection: "tasks", index: "open", equal: ["open"] },
    });
  });

  test("a closed database refuses find with closed", async () => {
    const { core } = fake();
    const db = await connect(core, { model, lock });
    await db.close();
    const refused = await db
      .find(model.tasks.indexes.byPriority)
      .catch((e) => e);
    expect(refused.code).toBe("closed");
  });
});

describe("subscriptions", () => {
  test("db.subscribe gives the page at once and again after a fold of its collection", async () => {
    let round = 0;
    const { core, requests, fold } = fake((r) =>
      r.call === "find"
        ? {
            ok: {
              items: [
                {
                  $id: TASK,
                  $space: SPACE,
                  $deleted: false,
                  fields: { priority: ++round },
                },
              ],
              cursor: null,
              hasMore: false,
            },
          }
        : undefined,
    );
    const db = await connect(core, { model, lock });
    const pages: Page<unknown>[] = [];
    db.subscribe(model.tasks.indexes.byPriority, (page) => pages.push(page));
    await settle();
    expect(pages.length).toBe(1);

    fold({ space: OTHER, collections: ["tasks"] });
    await settle();
    expect(pages.length).toBe(2);
    expect((pages[1]?.items[0] as { priority: number }).priority).toBe(2);
    expect(finds(requests).length).toBe(2);
  });

  test("a fold of another collection does not re-run it", async () => {
    const { core, requests, fold } = fake();
    const db = await connect(core, { model, lock });
    db.subscribe(model.tasks.indexes.byPriority, () => {});
    await settle();
    fold({ space: SPACE, collections: ["projects"] });
    await settle();
    expect(finds(requests).length).toBe(1);
  });

  test("folds that arrive together re-run it once", async () => {
    const { core, requests, fold } = fake();
    const db = await connect(core, { model, lock });
    db.subscribe(model.tasks.indexes.byPriority, () => {});
    await settle();
    fold(
      { space: SPACE, collections: ["tasks"] },
      { space: OTHER, collections: ["tasks"] },
    );
    fold({ space: SPACE, collections: ["tasks"] });
    await settle();
    expect(finds(requests).length).toBe(2);
  });

  test("the function it returns stops it", async () => {
    const { core, requests, fold } = fake();
    const db = await connect(core, { model, lock });
    const stop = db.subscribe(model.tasks.indexes.byPriority, () => {});
    await settle();
    stop();
    fold({ space: SPACE, collections: ["tasks"] });
    await settle();
    expect(finds(requests).length).toBe(1);
  });

  test("space.subscribe(query) re-runs only on folds in its space", async () => {
    const { core, requests, fold } = fake();
    const db = await connect(core, { model, lock });
    const project = await db.spaces.create();
    project.subscribe(model.tasks.indexes.open.equal("open"), () => {});
    await settle();
    fold({ space: OTHER, collections: ["tasks"] });
    await settle();
    expect(finds(requests).length).toBe(1);
    fold({ space: SPACE, collections: ["tasks"] });
    await settle();
    expect(finds(requests).length).toBe(2);
    expect(finds(requests)[1]?.args.space).toBe(SPACE);
  });

  test("space.subscribe(collection, id) follows one object as get reads it", async () => {
    let title = "a";
    const { core, fold } = fake((r) =>
      r.call === "get"
        ? {
            ok: {
              $id: TASK,
              $space: SPACE,
              $deleted: false,
              fields: { title },
            },
          }
        : undefined,
    );
    const db = await connect(core, { model, lock });
    const project = await db.spaces.create();
    const seen: (string | undefined)[] = [];
    project.subscribe(model.tasks, TASK, (task) => seen.push(task?.title));
    await settle();
    title = "b";
    fold({ space: SPACE, collections: ["tasks"] });
    await settle();
    expect(seen).toEqual(["a", "b"]);
  });

  test("closing the database stops every subscription", async () => {
    const { core, requests, fold } = fake();
    const db = await connect(core, { model, lock });
    db.subscribe(model.tasks.indexes.byPriority, () => {});
    await settle();
    await db.close();
    fold({ space: SPACE, collections: ["tasks"] });
    await settle();
    expect(finds(requests).length).toBe(1);
  });
});

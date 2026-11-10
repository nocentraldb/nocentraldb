import { describe, expect, test } from "vitest";
import { nc } from "../src/index.ts";

/** The key under which nc.model hides the description `ncdb model build` reads. */
const DESCRIBE = Symbol.for("nocentraldb.model.describe");

const draft = (model: object): unknown =>
  (model as Record<symbol, () => unknown>)[DESCRIBE]?.();

describe("the schema of RFC 0005 at model version 2", () => {
  const model = nc.model({
    people: nc.collection({
      name: nc.string().personal({ person: "self" }),
      email: nc.string().personal({ person: "self" }),
    }),
    tasks: nc.collection(
      {
        title: nc.string().describe("One line; what has to be done."),
        status: nc.oneOf(["open", "in_progress", "done"]),
        priority: nc.int(),
        estimate: nc.float(),
        archived: nc.boolean(),
        tags: nc.set(nc.string()),
        assignee: nc.link("people"),
        notesAboutAssignee: nc.string().personal({ person: "assignee" }),
        dependsOn: nc.links("tasks"),
      },
      {
        computed: {
          blocked: nc.reach("dependsOn", { status: ["open", "in_progress"] }),
        },
        views: {
          ready: nc.view("tasks", {
            where: { blocked: false },
            sort: ["priority"],
          }),
        },
        indexes: {
          byStatusPriority: nc.index(["status", "priority"], { equal: 1 }),
          byPriorityDesc: nc.index([{ field: "priority", direction: "desc" }]),
        },
      },
    ),
  });

  test("describes every collection, field, name and index for model build", () => {
    expect(draft(model)).toEqual({
      plugins: {},
      collections: {
        people: {
          fields: {
            name: {
              type: "ncdb/string",
              rule: 1,
              personal: { person: "self" },
            },
            email: {
              type: "ncdb/string",
              rule: 1,
              personal: { person: "self" },
            },
          },
          computed: [],
          views: [],
          indexes: {},
        },
        tasks: {
          fields: {
            title: {
              type: "ncdb/string",
              rule: 1,
              description: "One line; what has to be done.",
            },
            status: {
              type: "ncdb/choice",
              rule: 1,
              options: ["open", "in_progress", "done"],
            },
            priority: { type: "ncdb/int", rule: 1 },
            estimate: { type: "ncdb/float", rule: 1 },
            archived: { type: "ncdb/boolean", rule: 1 },
            tags: { type: "ncdb/set", rule: 1, of: { type: "ncdb/string" } },
            assignee: { type: "ncdb/link", rule: 1, targets: ["people"] },
            notesAboutAssignee: {
              type: "ncdb/string",
              rule: 1,
              personal: { person: "assignee" },
            },
            dependsOn: { type: "ncdb/links", rule: 1, targets: ["tasks"] },
          },
          computed: ["blocked"],
          views: ["ready"],
          indexes: {
            byStatusPriority: {
              equal: 1,
              fields: [
                { field: "status", direction: "asc" },
                { field: "priority", direction: "asc" },
              ],
            },
            byPriorityDesc: {
              equal: 0,
              fields: [{ field: "priority", direction: "desc" }],
            },
          },
        },
      },
    });
  });

  test("keeps the collections under their names, and the description out of sight", () => {
    expect(Object.keys(model)).toEqual(["people", "tasks"]);
    expect(JSON.parse(JSON.stringify(model))).not.toHaveProperty("plugins");
  });
});

describe("field functions and modifiers", () => {
  const fields = (
    collection: Parameters<typeof nc.model>[0][string],
  ): unknown =>
    (
      draft(nc.model({ things: collection })) as {
        collections: { things: { fields: unknown } };
      }
    ).collections.things.fields;

  test("a set takes a choice as its element, and links take several collections", () => {
    expect(
      fields(
        nc.collection({
          kinds: nc.set(nc.oneOf(["bug", "feature"])),
          related: nc.links(["tasks", "milestones"]),
          parent: nc.link(["tasks", "milestones"]),
        }),
      ),
    ).toEqual({
      kinds: {
        type: "ncdb/set",
        rule: 1,
        of: { type: "ncdb/choice", options: ["bug", "feature"] },
      },
      related: {
        type: "ncdb/links",
        rule: 1,
        targets: ["tasks", "milestones"],
      },
      parent: { type: "ncdb/link", rule: 1, targets: ["tasks", "milestones"] },
    });
  });

  test("deprecated keeps the field with its reason", () => {
    expect(
      fields(nc.collection({ priority: nc.int().deprecated("Use rank.") })),
    ).toEqual({
      priority: { type: "ncdb/int", rule: 1, deprecated: "Use rank." },
    });
  });

  test("a modifier returns a new field and leaves the one it was called on as it was", () => {
    const text = nc.string();
    const described = text.describe("Free text.");
    expect(
      fields(
        nc.collection({
          a: text,
          b: described,
          c: described.personal({ person: "self" }),
        }),
      ),
    ).toEqual({
      a: { type: "ncdb/string", rule: 1 },
      b: { type: "ncdb/string", rule: 1, description: "Free text." },
      c: {
        type: "ncdb/string",
        rule: 1,
        description: "Free text.",
        personal: { person: "self" },
      },
    });
  });
});

describe("a deprecated collection", () => {
  const people = nc.collection({ name: nc.string() });
  const model = nc.model({
    people: people.deprecated("Use members."),
    members: people,
  });
  const collections = (draft(model) as { collections: Record<string, unknown> })
    .collections;

  test("keeps the collection with its reason for model build", () => {
    expect(collections.people).toEqual({
      deprecated: "Use members.",
      fields: { name: { type: "ncdb/string", rule: 1 } },
      computed: [],
      views: [],
      indexes: {},
    });
  });

  test("returns a new collection and leaves the one it was called on as it was", () => {
    expect(collections.members).not.toHaveProperty("deprecated");
  });
});

describe("what model build refuses before the lock", () => {
  test("rich text and streams are reserved, not designed yet", () => {
    for (const reserved of [nc.richText(), nc.stream()]) {
      const model = nc.model({ tasks: nc.collection({ body: reserved }) });
      expect(() => draft(model)).toThrow(
        /tasks\.body: nc\.(richText|stream) is reserved/,
      );
    }
  });

  test("a set's element is a basic value without modifiers", () => {
    const model = (element: unknown) =>
      nc.model({
        tasks: nc.collection({
          tags: nc.set(element as ReturnType<typeof nc.string>),
        }),
      });
    expect(() => draft(model(nc.string().describe("x")))).toThrow(
      /tasks\.tags: a set's element takes no modifiers/,
    );
    expect(() => draft(model(nc.links("tasks")))).toThrow(
      /tasks\.tags: a set's element is a basic value/,
    );
  });

  test("something other than a field is refused with its name", () => {
    const model = nc.model({
      tasks: nc.collection({
        title: "string" as unknown as ReturnType<typeof nc.string>,
      }),
    });
    expect(() => draft(model)).toThrow(/tasks\.title: not a field/);
  });
});

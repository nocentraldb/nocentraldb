// nc.open checks the model against its lock (NOC-323; RFC 0009, "Open and
// close", 2. "Checks the model against the lock"): every collection,
// field and index the model names is in the lock with the same type and
// options. Against the lock the Rust SDK's test model builds, which the
// browser tests open with too. Chromium runs nc.open with an out-of-date
// lock (scripts/sdk-browser/model.e2e.test.mjs).
import { expect, test } from "vitest";
import lock from "../../../crates/nocentraldb/tests/model/model.lock.json" with {
  type: "json",
};
import { nc } from "../src/index.ts";
import { lockOutOfDate } from "../src/lockCheck.ts";

/** The model the lock was built from: crates/nocentraldb/tests/model/model.ts, as it declares it. */
const fullModel = nc.model({
  tasks: nc.collection(
    {
      title: nc.string(),
      status: nc.oneOf(["open", "done"]),
      priority: nc.int(),
      estimate: nc.float(),
      urgent: nc.boolean(),
      tags: nc.set(nc.string()),
      project: nc.link("projects"),
      customer: nc.ref("projects"),
      doneOn: nc.date(),
      dueAt: nc.timestamp(),
    },
    {
      indexes: {
        byPriority: nc.index(["priority"]),
        byStatusPriority: nc.index(["status", "priority"], { equal: 1 }),
        open: nc.index(["status", "priority"], { equal: 1, scope: "space" }),
        byDueAt: nc.index([{ field: "dueAt", direction: "desc" }]),
      },
    },
  ),
  projects: nc.collection({ name: nc.string() }),
});

/** The test model's tasks, with `fields` and `indexes` changed as a test asks. */
const tasks = (
  fields: Record<string, unknown> = {},
  indexes: Record<string, unknown> = {},
) =>
  nc.model({
    tasks: nc.collection(
      {
        title: nc.string(),
        status: nc.oneOf(["open", "done"]),
        priority: nc.int(),
        tags: nc.set(nc.string()),
        ...fields,
      } as Parameters<typeof nc.collection>[0],
      {
        indexes: {
          byPriority: nc.index(["priority"]),
          ...indexes,
        },
      } as Parameters<typeof nc.collection>[1],
    ),
  });

test("the model the lock was built from, and a part of it, match the lock", () => {
  expect(lockOutOfDate(fullModel, lock)).toBeUndefined();
  expect(lockOutOfDate(tasks(), lock)).toBeUndefined();
});

test("a collection, a field or an index the lock does not have is out of date, and the message says to build the lock", () => {
  const cases: [object, string][] = [
    [
      nc.model({ notes: nc.collection({ text: nc.string() }) }),
      "it has no collection notes",
    ],
    [tasks({ done: nc.boolean() }), "it has no field tasks.done"],
    [
      tasks({}, { byTitle: nc.index(["title"]) }),
      "it has no index tasks.byTitle",
    ],
  ];
  for (const [model, says] of cases) {
    const message = lockOutOfDate(model, lock);
    expect(message, says).toContain(says);
    expect(message).toContain("ncdb model build");
  }
});

test("a field of another type or with other options is out of date", () => {
  for (const [model, field] of [
    [tasks({ title: nc.int() }), "tasks.title"],
    [tasks({ status: nc.oneOf(["open", "done", "late"]) }), "tasks.status"],
    [tasks({ tags: nc.set(nc.int()) }), "tasks.tags"],
    [tasks({ doneOn: nc.timestamp() }), "tasks.doneOn"],
    [tasks({ dueAt: nc.date() }), "tasks.dueAt"],
    [
      tasks({ priority: nc.int().personal({ person: "self" }) }),
      "tasks.priority",
    ],
  ] as const) {
    expect(lockOutOfDate(model, lock), field).toContain(
      `${field} differs from the lock`,
    );
  }
});

test("an index with other fields, equality or scope is out of date", () => {
  for (const index of [
    nc.index(["priority", "title"]),
    nc.index([{ field: "priority", direction: "desc" }]),
    nc.index(["priority"], { equal: 1 }),
    nc.index(["priority"], { scope: "space" }),
  ]) {
    expect(lockOutOfDate(tasks({}, { byPriority: index }), lock)).toContain(
      "tasks.byPriority differs from the lock",
    );
  }
});

test("a description, which is no type or option, does not make the lock out of date", () => {
  expect(
    lockOutOfDate(tasks({ title: nc.string().describe("What to do") }), lock),
  ).toBeUndefined();
});

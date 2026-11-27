// The named index form (RFC 0018, R3; NOC-563): `nc.index({ where,
// orderBy })` names the fields a query fixes with equality and the fields
// that order the result, with "-" for a descending order. It is a second
// spelling of today's `nc.index(fields, { equal })`: both describe the
// same index for `ncdb model build`, so both build the same lock entry.
// The build refuses a named index that names a field twice, a field in
// both `where` and `orderBy`, or no field at all.
import { describe, expect, test } from "vitest";
import { nc } from "../src/index.ts";

/** The key under which nc.model hides the description `ncdb model build` reads. */
const DESCRIBE = Symbol.for("nocentraldb.model.describe");

const draft = (model: object): unknown =>
  (model as Record<symbol, () => unknown>)[DESCRIBE]?.();

const fields = {
  title: nc.string(),
  status: nc.oneOf(["open", "done"]),
  priority: nc.int(),
  occurredOn: nc.date(),
};

/** What model build reads of the indexes of a `tasks` collection with `indexes`. */
const described = (indexes: Record<string, unknown>) =>
  (
    draft(
      nc.model({
        tasks: nc.collection(fields, {
          indexes: indexes as Record<string, ReturnType<typeof nc.index>>,
        }),
      }),
    ) as { collections: { tasks: { indexes: unknown } } }
  ).collections.tasks.indexes;

describe("the named form describes the index today's form describes", () => {
  test("where fields, each ascending, then the orderBy fields, with equal the count of where", () => {
    expect(
      described({
        byStatusPriority: nc.index({
          where: ["status"],
          orderBy: ["priority"],
        }),
      }),
    ).toEqual(
      described({
        byStatusPriority: nc.index(["status", "priority"], { equal: 1 }),
      }),
    );
  });

  test('a leading "-" sorts an orderBy field descending', () => {
    expect(
      described({ latest: nc.index({ orderBy: ["-occurredOn", "title"] }) }),
    ).toEqual(
      described({
        latest: nc.index([{ field: "occurredOn", direction: "desc" }, "title"]),
      }),
    );
  });

  test("without orderBy, the index is equality only; without where, a sort only", () => {
    expect(
      described({
        byStatus: nc.index({ where: ["status", "priority"] }),
        byPriority: nc.index({ orderBy: ["priority"] }),
      }),
    ).toEqual(
      described({
        byStatus: nc.index(["status", "priority"], { equal: 2 }),
        byPriority: nc.index(["priority"]),
      }),
    );
  });

  test('scope: "space" stays as it is, and empty lists count as left out', () => {
    expect(
      described({
        open: nc.index({
          where: ["status"],
          orderBy: ["-priority"],
          scope: "space",
        }),
        sorted: nc.index({ where: [], orderBy: ["title"] }),
      }),
    ).toEqual(
      described({
        open: nc.index(["status", { field: "priority", direction: "desc" }], {
          equal: 1,
          scope: "space",
        }),
        sorted: nc.index(["title"]),
      }),
    );
  });
});

describe("what model build refuses in a named index", () => {
  test("a field named twice in where", () => {
    expect(() =>
      described({ twice: nc.index({ where: ["status", "status"] }) }),
    ).toThrow(
      "tasks.twice: the index names the field status twice; name each field once.",
    );
  });

  test("a field named twice in orderBy, whatever its direction", () => {
    expect(() =>
      described({
        twice: nc.index({ orderBy: ["priority", "-priority"] }),
      }),
    ).toThrow(
      "tasks.twice: the index names the field priority twice; name each field once.",
    );
  });

  test("a field in both where and orderBy", () => {
    expect(() =>
      described({
        both: nc.index({ where: ["status"], orderBy: ["-status"] }),
      }),
    ).toThrow(
      "tasks.both: the field status is in both where and orderBy; a query fixes a where field, so it cannot order the result.",
    );
  });

  test("an index that names no field", () => {
    expect(() =>
      described({ none: nc.index({ where: [], orderBy: [] }) }),
    ).toThrow(
      "tasks.none: the index names no field; give where, orderBy or both.",
    );
  });
});

describe("the types of the named form", () => {
  test("where fixes the where fields, in count and type", () => {
    const model = nc.model({
      tasks: nc.collection(fields, {
        indexes: {
          byStatusPriority: nc.index({
            where: ["status"],
            orderBy: ["-priority"],
          }),
          byPriority: nc.index({ orderBy: ["priority"] }),
        },
      }),
    });
    model.tasks.indexes.byStatusPriority.where("open");
    model.tasks.indexes.byStatusPriority.equal("open");
    // @ts-expect-error: the index fixes one field
    model.tasks.indexes.byStatusPriority.where("open", 2);
    // @ts-expect-error: status is a choice of "open" or "done"
    model.tasks.indexes.byStatusPriority.where(3);
    // @ts-expect-error: byPriority fixes no field
    model.tasks.indexes.byPriority.where(1);
  });
});

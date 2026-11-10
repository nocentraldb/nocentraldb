// nc.Infer as RFC 0005 defines it ("Types for the app", Amendments 3 and
// 4): every field optional, a choice as the union of its options and
// nc.Unknown, an nc.int as a number (D16), a set as an array, a link as an
// object id, a reference as { space, id } (RFC 0009, Amendment 4), and nothing of a deprecated field or collection (D17). These
// are type-level tests: `tsc` (the typecheck gate) fails when they do not
// hold, and vitest runs them as no-ops.
import { describe, expectTypeOf, test } from "vitest";
import { nc } from "../src/index.ts";

const model = nc.model({
  people: nc.collection({
    name: nc.string().personal({ person: "self" }),
  }),
  teams: nc.collection({ name: nc.string() }).deprecated("Use groups."),
  tasks: nc.collection({
    title: nc.string(),
    status: nc.oneOf(["open", "in_progress", "done"]),
    priority: nc.int(),
    estimate: nc.float(),
    archived: nc.boolean(),
    tags: nc.set(nc.string()),
    kinds: nc.set(nc.oneOf(["bug", "feature"])),
    sizes: nc.set(nc.int()),
    assignee: nc.link("people"),
    notesAboutAssignee: nc.string().personal({ person: "assignee" }),
    dependsOn: nc.links("tasks"),
    customer: nc.ref("customers"),
    oldCustomer: nc.ref("customers").deprecated("Use customer."),
    rank: nc.int().describe("Higher first."),
    old: nc.boolean().deprecated("Use archived."),
  }),
});

type Task = nc.Infer<typeof model.tasks>;

describe("nc.Infer types every field as RFC 0005 defines", () => {
  test("basic values, every one optional", () => {
    expectTypeOf<Task["title"]>().toEqualTypeOf<string | undefined>();
    expectTypeOf<Task["estimate"]>().toEqualTypeOf<number | undefined>();
    expectTypeOf<Task["archived"]>().toEqualTypeOf<boolean | undefined>();
    expectTypeOf<Record<never, never>>().toExtend<Task>();
  });

  test("an nc.int as a number (D16)", () => {
    expectTypeOf<Task["priority"]>().toEqualTypeOf<number | undefined>();
    expectTypeOf<Task["rank"]>().toEqualTypeOf<number | undefined>();
  });

  test("a choice as the union of its options and nc.Unknown", () => {
    expectTypeOf<Task["status"]>().toEqualTypeOf<
      "open" | "in_progress" | "done" | nc.Unknown | undefined
    >();
    expectTypeOf<nc.Unknown>().toEqualTypeOf<{ unknown: string }>();
  });

  test("a set as an array of its elements", () => {
    expectTypeOf<Task["tags"]>().toEqualTypeOf<string[] | undefined>();
    expectTypeOf<Task["kinds"]>().toEqualTypeOf<
      ("bug" | "feature" | nc.Unknown)[] | undefined
    >();
    expectTypeOf<Task["sizes"]>().toEqualTypeOf<number[] | undefined>();
  });

  test("a link as an object id, many links as an array of ids", () => {
    expectTypeOf<Task["assignee"]>().toEqualTypeOf<string | undefined>();
    expectTypeOf<Task["dependsOn"]>().toEqualTypeOf<string[] | undefined>();
  });

  test("a reference as its target's space id and object id, without its state", () => {
    expectTypeOf<Task["customer"]>().toEqualTypeOf<
      { space: string; id: string } | undefined
    >();
  });

  test("a personal field as its value", () => {
    expectTypeOf<Task["notesAboutAssignee"]>().toEqualTypeOf<
      string | undefined
    >();
    expectTypeOf<nc.Infer<typeof model.people>>().toEqualTypeOf<{
      name?: string;
    }>();
  });
});

describe("deprecation hides from the app's types (D17)", () => {
  test("a deprecated field is not in nc.Infer", () => {
    expectTypeOf<Task>().not.toHaveProperty("old");
    expectTypeOf<Task>().not.toHaveProperty("oldCustomer");
  });

  test("a deprecated collection is not in the model's type", () => {
    expectTypeOf(model).not.toHaveProperty("teams");
    expectTypeOf(model).toHaveProperty("tasks");
  });
});

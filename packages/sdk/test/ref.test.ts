// nc.ref, a reference into another space (RFC 0005, Amendment 5): one
// collection or a list of them, as nc.link takes them (D18), and only the
// modifiers .describe and .deprecated (D22).
import { describe, expect, expectTypeOf, test } from "vitest";
import { nc } from "../src/index.ts";

/** The key under which nc.model hides the description `ncdb model build` reads. */
const DESCRIBE = Symbol.for("nocentraldb.model.describe");

const draft = (model: object): unknown =>
  (model as Record<symbol, () => unknown>)[DESCRIBE]?.();

const fields = (collection: Parameters<typeof nc.model>[0][string]): unknown =>
  (
    draft(nc.model({ orders: collection })) as {
      collections: { orders: { fields: unknown } };
    }
  ).collections.orders.fields;

describe("nc.ref", () => {
  test("is described as ncdb/ref with its targets, one collection or a list", () => {
    expect(
      fields(
        nc.collection({
          customer: nc.ref("customers"),
          buyer: nc.ref(["customers", "leads"]),
        }),
      ),
    ).toEqual({
      customer: { type: "ncdb/ref", rule: 1, targets: ["customers"] },
      buyer: { type: "ncdb/ref", rule: 1, targets: ["customers", "leads"] },
    });
  });

  test("takes .describe and .deprecated (D22)", () => {
    expect(
      fields(
        nc.collection({
          customer: nc.ref("customers").describe("Who ordered."),
          oldCustomer: nc.ref("customers").deprecated("Use customer."),
        }),
      ),
    ).toEqual({
      customer: {
        type: "ncdb/ref",
        rule: 1,
        targets: ["customers"],
        description: "Who ordered.",
      },
      oldCustomer: {
        type: "ncdb/ref",
        rule: 1,
        targets: ["customers"],
        deprecated: "Use customer.",
      },
    });
  });

  test("cannot be personal (D22)", () => {
    const reference = nc.ref("customers");
    expectTypeOf(reference).not.toHaveProperty("personal");
    expectTypeOf(reference.describe("x")).not.toHaveProperty("personal");
    expect(reference).not.toHaveProperty("personal");
    expect(reference.describe("x")).not.toHaveProperty("personal");
  });

  test("is no set element: there is no field of many references (D22)", () => {
    const model = nc.model({
      orders: nc.collection({
        customers: nc.set(
          nc.ref("customers") as unknown as ReturnType<typeof nc.string>,
        ),
      }),
    });
    expect(() => draft(model)).toThrow(
      /orders\.customers: a set's element is a basic value/,
    );
  });
});

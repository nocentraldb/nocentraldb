// A rich-text field's vocabulary in the schema (RFC 0013, "The field in the
// schema and the lock"; RFC 0005, "Field types"; NOC-553):
// `nc.richText({ vocabulary: { nodes, marks } })` describes the names its
// editor uses to `ncdb model build`, and nc.open refuses a lock whose
// vocabulary differs from the model's, as for any field changed without a
// build (NOC-323).
import { describe, expect, test } from "vitest";
import { nc } from "../src/index.ts";
import { lockOutOfDate } from "../src/lockCheck.ts";

/** The key under which nc.model hides the description `ncdb model build` reads. */
const DESCRIBE = Symbol.for("nocentraldb.model.describe");

const notes = (body: Parameters<typeof nc.collection>[0][string]) =>
  nc.model({ notes: nc.collection({ body }) });

const described = (model: object): unknown =>
  (
    (model as Record<symbol, () => unknown>)[DESCRIBE]?.() as {
      collections: { notes: { fields: { body: unknown } } };
    }
  ).collections.notes.fields.body;

describe("the schema", () => {
  test("describes the vocabulary as the schema lists it", () => {
    expect(
      described(
        notes(
          nc.richText({
            vocabulary: {
              nodes: ["paragraph", "heading", "callout"],
              marks: ["bold", "link"],
            },
          }),
        ),
      ),
    ).toEqual({
      type: "ncdb/richText",
      rule: 1,
      vocabulary: {
        nodes: ["paragraph", "heading", "callout"],
        marks: ["bold", "link"],
      },
    });
  });

  test("a missing list is empty, and without the option there is no vocabulary", () => {
    expect(
      described(notes(nc.richText({ vocabulary: { nodes: ["paragraph"] } }))),
    ).toEqual({
      type: "ncdb/richText",
      rule: 1,
      vocabulary: { nodes: ["paragraph"], marks: [] },
    });
    expect(described(notes(nc.richText()))).toEqual({
      type: "ncdb/richText",
      rule: 1,
    });
  });

  test("the vocabulary stays through describe and deprecated", () => {
    const vocabulary = { nodes: ["paragraph"], marks: ["bold"] };
    expect(
      described(notes(nc.richText({ vocabulary }).describe("The text."))),
    ).toEqual({
      type: "ncdb/richText",
      rule: 1,
      vocabulary,
      description: "The text.",
    });
    expect(
      described(notes(nc.richText({ vocabulary }).deprecated("Use body2."))),
    ).toEqual({
      type: "ncdb/richText",
      rule: 1,
      vocabulary,
      deprecated: "Use body2.",
    });
  });

  test("the schema's lists are copies, so a later change to the app's array changes nothing", () => {
    const nodes = ["paragraph"];
    const model = notes(nc.richText({ vocabulary: { nodes } }));
    nodes.push("callout");
    expect(described(model)).toEqual({
      type: "ncdb/richText",
      rule: 1,
      vocabulary: { nodes: ["paragraph"], marks: [] },
    });
  });
});

describe("nc.open's check against the lock", () => {
  const lockWith = (vocabulary?: object) => ({
    collections: {
      notes: {
        fields: {
          body: {
            rule: 1,
            since: 1,
            type: "ncdb/richText",
            ...(vocabulary === undefined ? {} : { vocabulary }),
          },
        },
        since: 1,
      },
    },
  });
  const body = nc.richText({
    vocabulary: {
      nodes: ["paragraph", "callout", "paragraph"],
      marks: ["link", "bold"],
    },
  });

  test("passes when the lock holds the same names, in its sorted form", () => {
    expect(
      lockOutOfDate(
        notes(body),
        lockWith({ marks: ["bold", "link"], nodes: ["callout", "paragraph"] }),
      ),
    ).toBeUndefined();
  });

  test("an empty vocabulary matches a lock entry without the key", () => {
    expect(lockOutOfDate(notes(nc.richText()), lockWith())).toBeUndefined();
    expect(
      lockOutOfDate(
        notes(nc.richText({ vocabulary: { nodes: [], marks: [] } })),
        lockWith(),
      ),
    ).toBeUndefined();
  });

  test("refuses a lock whose vocabulary differs, as when the lock was not built again", () => {
    for (const vocabulary of [
      undefined,
      { marks: ["bold"], nodes: ["callout", "paragraph"] },
      { marks: ["bold", "link"], nodes: ["callout", "heading", "paragraph"] },
    ]) {
      expect(lockOutOfDate(notes(body), lockWith(vocabulary))).toContain(
        "notes.body differs from the lock",
      );
    }
  });
});

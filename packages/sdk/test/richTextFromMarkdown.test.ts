// `nc.richText.fromMarkdown(markdown)` (RFC 0013, "The first content";
// NOC-578): a value that writes a first document from Markdown into a
// rich-text field, on create or update; the SDK sends it to the core as
// `{ $markdown }`, and the core reads it into a document as it does for the
// Rust SDK's `RichText::from_markdown`, so the Markdown goes as written. A
// read gives the field's Markdown, a string (NOC-589). On a core without
// rich text, a write that carries one is refused with `notSupported`, as
// `space.richText` is (Carsten, 2026-11-28, NOC-590 2a). Against a fake core;
// scripts/sdk-browser/richText.e2e.test.mjs runs the shared table of cases
// through the real core in Chromium.
import { describe, expect, expectTypeOf, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { nc } from "../src/index.ts";

const model = nc.model({
  notes: nc.collection({ title: nc.string(), body: nc.richText() }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };
const SPACE = "0123456789abcdef0123456789abcdef";
const NOTE = "fedcba9876543210fedcba9876543210";

type Request = { id: number; call: string; args: Record<string, unknown> };

const fake = (richText = true) => {
  const requests: Request[] = [];
  const core: Core = {
    call: async (bytes) => {
      const request = decode(bytes) as Request;
      requests.push(request);
      const ok = (() => {
        switch (request.call) {
          case "open":
            return { sync: null, richText };
          case "spaces.create":
          case "spaces.get":
            return SPACE;
          case "write":
            return { change: "aa".repeat(32), ids: [NOTE] };
          case "get":
            return {
              $id: NOTE,
              $space: SPACE,
              $deleted: false,
              fields: { title: "Plan", body: "## Plan\n\nShip **it**." },
            };
          default:
            return null;
        }
      })();
      return encode({ id: request.id, ok });
    },
    terminate: () => {},
    listen: () => {},
  };
  const writes = () =>
    requests
      .filter((r) => r.call === "write")
      .map((r) => (r.args.ops as { values: Record<string, unknown> }[])[0]);
  return { core, writes };
};

describe("nc.richText.fromMarkdown", () => {
  test("a create sends the Markdown as $markdown for the field", async () => {
    const { core, writes } = fake();
    const db = await connect(core, { model, lock });
    const space = await db.spaces.create();
    await space.create(model.notes, {
      title: "Plan",
      body: nc.richText.fromMarkdown("## Plan\n\nShip **it**."),
    });
    expect(writes()[0]?.values).toEqual({
      title: "Plan",
      body: { $markdown: "## Plan\n\nShip **it**." },
    });
  });

  test("an update sends it too", async () => {
    const { core, writes } = fake();
    const db = await connect(core, { model, lock });
    const space = await db.spaces.create();
    await space.update(model.notes, NOTE, {
      body: nc.richText.fromMarkdown("First."),
    });
    expect(writes()[0]?.values).toEqual({ body: { $markdown: "First." } });
  });

  test("the Markdown goes as written, as the Rust SDK sends it", async () => {
    const { core, writes } = fake();
    const db = await connect(core, { model, lock });
    const space = await db.spaces.create();
    const decomposed = "Café";
    await space.create(model.notes, {
      body: nc.richText.fromMarkdown(decomposed),
    });
    expect(writes()[0]?.values).toEqual({ body: { $markdown: decomposed } });
  });

  test("a read gives the field's Markdown, a string", async () => {
    const { core } = fake();
    const db = await connect(core, { model, lock });
    const space = await db.spaces.get(SPACE);
    const note = await space.get(model.notes, NOTE);
    expect(note?.body).toBe("## Plan\n\nShip **it**.");
    expectTypeOf(note?.body).toEqualTypeOf<string | undefined>();
  });

  test("a core without rich text refuses a create or an update that carries one with notSupported, as space.richText does", async () => {
    const { core, writes } = fake(false);
    const db = await connect(core, { model, lock });
    const space = await db.spaces.create();
    await expect(
      space.create(model.notes, {
        title: "Plan",
        body: nc.richText.fromMarkdown("Plan."),
      }),
    ).rejects.toMatchObject({ code: "notSupported" });
    await expect(
      space.update(model.notes, NOTE, {
        body: nc.richText.fromMarkdown("Plan."),
      }),
    ).rejects.toMatchObject({ code: "notSupported" });
    await expect(
      space.richText(model.notes, NOTE, "body"),
    ).rejects.toMatchObject({ code: "notSupported" });
    expect(writes()).toEqual([]);
  });

  test("a rich-text field takes a first document, and a string field does not", async () => {
    const { core } = fake();
    const db = await connect(core, { model, lock });
    const space = await db.spaces.create();
    const first = nc.richText.fromMarkdown("Text");
    const write = () =>
      space.create(model.notes, {
        // @ts-expect-error: a rich-text field takes no whole document as a string (RFC 0013, "Writing")
        body: "Text",
        // @ts-expect-error: only a rich-text field takes a first document
        title: first,
      });
    expect(write).toBeTypeOf("function");
  });
});

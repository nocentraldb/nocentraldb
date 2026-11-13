// Invitations and members (RFC 0009, "Invitations and members"; NOC-176
// slice 4b). A member's invitation is its text and, for this member's own,
// its account's fingerprint; `nc.invitation(text)` reads one back. A
// space's members are added by invitation, listed, re-roled and removed by
// their id, 64 lowercase hex digits (NOC-298). The shapes RFC 0009 leaves
// open follow NOC-305 A and A (Carsten, 2026-11-13). Against a fake core.
import { expect, expectTypeOf, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { NcError } from "../src/error.ts";
import { nc } from "../src/index.ts";

const model = nc.model({
  tasks: nc.collection({ title: nc.string() }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };
const SPACE = "0123456789abcdef0123456789abcdef";
const MEMBER = "ab".repeat(32);
const TEXT = "ncdb-invite-1:AQIDBA";
const FINGERPRINT =
  "12345 67890 12345 67890 12345 67890 12345 67890 12345 67890 12345 67890";
const ADDED = {
  code: "memberAdded",
  text: "The new member reads the whole history of this space.",
};

type Call = { call: string; args: Record<string, unknown> };

/** A fake core that answers the membership calls, and `members.add` with `add`. */
const fake = (
  add: (args: Record<string, unknown>) => Record<string, unknown> = () => ({
    ok: { notice: ADDED },
  }),
) => {
  const calls: Call[] = [];
  const core: Core = {
    call: async (bytes) => {
      const { id, call, args } = decode(bytes) as Call & { id: number };
      calls.push({ call, args });
      const ok = (value: unknown) => encode({ id, ok: value });
      switch (call) {
        case "invitation":
          return ok({ text: TEXT, fingerprint: FINGERPRINT });
        case "spaces.create":
          return ok(SPACE);
        case "members.add":
          return encode({ id, ...add(args) });
        case "members.list":
          return ok([
            {
              id: MEMBER,
              account: FINGERPRINT,
              kind: "device",
              role: "admin",
              self: true,
            },
          ]);
        case "members.remove":
          return ok({ notice: { code: "memberRemoved", text: "kept" } });
        case "members.setRole":
          return ok({ notice: { code: "roleSet", text: "kept" } });
        default:
          return ok(null);
      }
    },
    terminate: () => {},
    listen: () => {},
  };
  return { core, calls };
};

test("db.invitation() gives this member's invitation: its text and its account's fingerprint", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock });
  const invitation = await db.invitation();
  expect(invitation.toString()).toBe(TEXT);
  expect(String(invitation)).toBe(TEXT);
  expect(invitation.fingerprint).toBe(FINGERPRINT);
});

test("nc.invitation(text) reads an invitation's text back, without its fingerprint", () => {
  const invitation = nc.invitation(` ${TEXT}\n`);
  expect(invitation.toString()).toBe(TEXT);
  expect(invitation.fingerprint).toBeUndefined();
});

test("nc.invitation refuses a text that is not an invitation with invalidValue", () => {
  for (const text of [
    "",
    "hello",
    "ncdb-invite-1:",
    "ncdb-invite-1:a+b/",
    "ncdb-invite-2:AQID",
  ]) {
    let thrown: unknown;
    try {
      nc.invitation(text);
    } catch (error) {
      thrown = error;
    }
    expect(thrown, text).toBeInstanceOf(NcError);
    expect((thrown as NcError).code, text).toBe("invalidValue");
  }
});

test("members.add sends the space, the invitations' texts, the role and the fingerprint, and gives the notice", async () => {
  const { core, calls } = fake();
  const db = await connect(core, { model, lock });
  const project = await db.spaces.create();
  const added = await project.members.add(nc.invitation(TEXT), {
    role: "writer",
    fingerprint: FINGERPRINT,
  });
  expect(added.notice).toEqual(ADDED);
  expect(calls.at(-1)).toEqual({
    call: "members.add",
    args: {
      space: SPACE,
      invitations: [TEXT],
      role: "writer",
      fingerprint: FINGERPRINT,
    },
  });
  await project.members.add([nc.invitation(TEXT), await db.invitation()]);
  expect(calls.at(-1)?.args).toEqual({
    space: SPACE,
    invitations: [TEXT, TEXT],
  });
});

test("an account new to this device fails with unknownAccount, carrying the fingerprint to compare", async () => {
  const { core } = fake(() => ({
    error: {
      code: "unknownAccount",
      message: "compare the fingerprint",
      fingerprint: FINGERPRINT,
    },
  }));
  const db = await connect(core, { model, lock });
  const project = await db.spaces.create();
  const refused = await project.members
    .add(nc.invitation(TEXT), { role: "writer" })
    .catch((error: unknown) => error);
  expect(refused).toBeInstanceOf(NcError);
  expect((refused as NcError).code).toBe("unknownAccount");
  expect((refused as NcError).fingerprint).toBe(FINGERPRINT);
});

test("members.list gives each member's id, account, kind, role and whether it is this member", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock });
  const project = await db.spaces.create();
  expect(await project.members.list()).toEqual([
    {
      id: MEMBER,
      account: FINGERPRINT,
      kind: "device",
      role: "admin",
      self: true,
    },
  ]);
});

test("members.setRole and members.remove name a member by its id, and give their notice", async () => {
  const { core, calls } = fake();
  const db = await connect(core, { model, lock });
  const project = await db.spaces.create();
  const lowered = await project.members.setRole(MEMBER, "reader");
  expect(lowered.notice.code).toBe("roleSet");
  expect(calls.at(-1)).toEqual({
    call: "members.setRole",
    args: { space: SPACE, member: MEMBER, role: "reader" },
  });
  const removed = await project.members.remove(MEMBER);
  expect(removed.notice.code).toBe("memberRemoved");
  expect(calls.at(-1)).toEqual({
    call: "members.remove",
    args: { space: SPACE, member: MEMBER },
  });
});

test("their types are RFC 0009's", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock });
  const project = await db.spaces.create();
  const [member] = await project.members.list();
  expectTypeOf(member?.role).toEqualTypeOf<
    "reader" | "writer" | "admin" | null | undefined
  >();
  expectTypeOf(member?.kind).toEqualTypeOf<
    "device" | "agent" | "subAgent" | undefined
  >();
  expectTypeOf(project.members.setRole)
    .parameter(1)
    .toEqualTypeOf<"reader" | "writer" | "admin">();
  const added = await project.members.add(nc.invitation(TEXT), {
    role: "admin",
  });
  expectTypeOf(added.notice.code).toEqualTypeOf<string>();
});

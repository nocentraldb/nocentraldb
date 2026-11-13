// A space's members and this member's invitation over the bridge (NOC-176
// slice 4b; crates/nocentraldb/src/bridge/members.rs). Internal: the
// package's entry does not reach it, so it is not in the API report.

import type { Invitation, Members, Notice } from "./members.ts";

/** This member's invitation, as the core's `invitation` call answers it. */
export const ownInvitation = (answer: unknown): Invitation => {
  const { text, fingerprint } = answer as { text: string; fingerprint: string };
  return {
    fingerprint,
    toString: () => text,
    toJSON: () => text,
  } as Invitation;
};

/** The members of the space `space`, through `ask`. */
export const membersOf = (
  ask: (call: string, args: Record<string, unknown>) => Promise<unknown>,
  space: string,
): Members => ({
  add: async (invitations, options = {}) => {
    const list = Array.isArray(invitations) ? invitations : [invitations];
    return (await ask("members.add", {
      space,
      invitations: (list as readonly Invitation[]).map(String),
      ...(options.role === undefined ? {} : { role: options.role }),
      ...(options.fingerprint === undefined
        ? {}
        : { fingerprint: options.fingerprint }),
    })) as { notice: Notice };
  },
  list: async () =>
    (await ask("members.list", { space })) as Awaited<
      ReturnType<Members["list"]>
    >,
  setRole: async (member, role) =>
    (await ask("members.setRole", { space, member, role })) as {
      notice: Notice;
    },
  remove: async (member) =>
    (await ask("members.remove", { space, member })) as { notice: Notice },
});

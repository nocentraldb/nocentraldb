// A database whose storage the browser dropped is reported, not reopened
// empty in silence (NOC-445). In a Chromium private window the origin's
// OPFS goes after about ten minutes with no database open; IndexedDB
// stays. The SDK records each database it opened in IndexedDB, outside
// OPFS, and a twin of that record on OPFS. When `nc.open` finds the
// record without its twin, the files are gone: it refuses with
// `storageLost`, unless the app opens with `ifStorageLost: "open-empty"`.
import { expect, test, vi } from "vitest";
import { NcError } from "../src/error.ts";
import { guardLoss, type Markers } from "../src/storageLost.ts";

/** Markers in memory: `records` outside OPFS, `twins` on it. */
const markers = (records: string[] = [], twins: string[] = []) => {
  const outside = new Set(records);
  const opfs = new Set(twins);
  const fake: Markers & { outside: Set<string>; opfs: Set<string> } = {
    outside,
    opfs,
    recorded: async (name) => outside.has(name),
    present: async (name) => opfs.has(name),
    record: async (name) => {
      opfs.add(name);
      outside.add(name);
    },
  };
  return fake;
};

const opened = () => vi.fn(async () => "db");

test("a database opened for the first time opens, and is recorded on both sides", async () => {
  const store = markers();
  const open = opened();
  await expect(guardLoss(store, "default", undefined, open)).resolves.toBe(
    "db",
  );
  expect(open).toHaveBeenCalledTimes(1);
  expect([...store.outside]).toEqual(["default"]);
  expect([...store.opfs]).toEqual(["default"]);
});

test("a database whose record and twin are both there opens", async () => {
  const open = opened();
  await expect(
    guardLoss(markers(["default"], ["default"]), "default", undefined, open),
  ).resolves.toBe("db");
  expect(open).toHaveBeenCalledTimes(1);
});

test("a database recorded outside OPFS but gone from it is refused with storageLost, before the core opens", async () => {
  const open = opened();
  const refused = guardLoss(markers(["default"], []), "default", undefined, open);
  await expect(refused).rejects.toBeInstanceOf(NcError);
  await expect(refused).rejects.toMatchObject({ code: "storageLost" });
  expect(open).not.toHaveBeenCalled();
});

test("the refusal names the database and how to go on", async () => {
  await expect(
    guardLoss(markers(["notes"], []), "notes", "refuse", opened()),
  ).rejects.toThrow(/notes[\s\S]*ifStorageLost: "open-empty"/);
});

test("refusing is the default, and the refusal repeats until the app accepts the loss", async () => {
  const store = markers(["default"], []);
  for (const ifLost of [undefined, "refuse", undefined] as const) {
    await expect(
      guardLoss(store, "default", ifLost, opened()),
    ).rejects.toMatchObject({ code: "storageLost" });
  }
});

test('with ifStorageLost: "open-empty" a lost database opens empty, and the next open is a normal one', async () => {
  const store = markers(["default"], []);
  const open = opened();
  await expect(guardLoss(store, "default", "open-empty", open)).resolves.toBe(
    "db",
  );
  expect(open).toHaveBeenCalledTimes(1);
  await expect(
    guardLoss(store, "default", undefined, opened()),
  ).resolves.toBe("db");
});

test("the loss of one database does not touch another", async () => {
  const store = markers(["lost", "kept"], ["kept"]);
  await expect(
    guardLoss(store, "kept", undefined, opened()),
  ).resolves.toBe("db");
  await expect(
    guardLoss(store, "lost", undefined, opened()),
  ).rejects.toMatchObject({ code: "storageLost" });
});

test("an open that fails records nothing", async () => {
  const store = markers();
  const failing = vi.fn(async () => {
    throw new NcError("storageUnavailable", "no OPFS");
  });
  await expect(
    guardLoss(store, "default", undefined, failing),
  ).rejects.toMatchObject({ code: "storageUnavailable" });
  expect(store.outside.size).toBe(0);
  expect(store.opfs.size).toBe(0);
});

test("markers that cannot be read leave the open to the core, whose own error then says what is missing", async () => {
  const broken: Markers = {
    recorded: async () => {
      throw new Error("no IndexedDB");
    },
    present: async () => {
      throw new Error("no OPFS");
    },
    record: async () => {
      throw new Error("no OPFS");
    },
  };
  const open = opened();
  await expect(guardLoss(broken, "default", undefined, open)).resolves.toBe(
    "db",
  );
  expect(open).toHaveBeenCalledTimes(1);
});

test("without markers, as outside a browser, the open goes on as before", async () => {
  const open = opened();
  await expect(guardLoss(undefined, "default", undefined, open)).resolves.toBe(
    "db",
  );
});

// The latency of a pause-based write, over the real TypeScript SDK: between
// two tabs of one browser, which share one database on OPFS (RFC 0012), and
// between two browsers, two Chromium processes each with its own database,
// syncing through the local relay server (target/sdk-browser/ncdb-relay).
// Latency is from the last key the writer pressed to the moment the other
// side's binding holds the text, both read from performance.timeOrigin +
// performance.now(), one clock on one machine.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { launch, openPage, sleep } from "./browser.mjs";
import { ROOT } from "./server.mjs";

const LOCK = JSON.parse(
  readFileSync(
    join(ROOT, "crates", "nocentraldb", "tests", "model", "model.lock.json"),
    "utf8",
  ),
);

/** The test lock's model, as crates/nocentraldb/tests/model/model.ts declares it. */
const MODEL = `nc.model({
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
})`;

const openDb = (page, name, relay) =>
  page.evaluate(
    async ({ lock, MODEL, name, relay }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({
        model,
        lock,
        name,
        ...(relay ? { relay } : {}),
      });
      window.tab = { db, model };
      if (relay) {
        for (
          let i = 0;
          i < 600 && db.sync.current().connection !== "connected";
          i += 1
        ) {
          await new Promise((r) => setTimeout(r, 50));
        }
      }
    },
    { lock: LOCK, MODEL, name, relay },
  );

/** Clicks into the editor and puts the cursor at its end, as a person would. */
const intoEditor = async (page) => {
  await page.bringToFront();
  await page.click("#editor .tiptap");
  await page.keyboard.press("Control+End");
};

const waitFor = async (what, check, ms = 30000) => {
  const start = Date.now();
  for (;;) {
    if (await check()) return;
    if (Date.now() - start > ms) throw new Error(`${what}: not in time`);
    await sleep(50);
  }
};

const stats = (xs) => {
  const s = [...xs].sort((a, b) => a - b);
  const q = (p) => s[Math.min(s.length - 1, Math.floor(p * s.length))];
  return {
    n: s.length,
    min: Math.round(s[0]),
    median: Math.round(q(0.5)),
    p90: Math.round(q(0.9)),
    max: Math.round(s.at(-1)),
  };
};

/** Writer A types a short word, pauses; how long until B holds it. */
const bursts = async (a, b, trials) => {
  await a.bringToFront();
  const rows = [];
  for (let i = 0; i < trials; i += 1) {
    const token = `w${i}x`;
    await b.evaluate((t) => {
      window.seen = window.spike.whenHolds(t);
    }, token);
    await intoEditor(a);
    await a.keyboard.type(` ${token}`, { delay: 40 });
    const seenAt = await b.evaluate(() => window.seen);
    await sleep(300);
    const lastKey = await a.evaluate(() => window.spike.lastKeyAt());
    const w = (await a.evaluate(() => window.spike.writes())).at(-1);
    rows.push({
      total: seenAt - lastKey,
      pause: w.startedAt - lastKey,
      write: w.doneAt - w.startedAt,
      fromWriteToOther: seenAt - w.startedAt,
    });
    await sleep(500);
  }
  const visibility = {
    writer: await a.evaluate(() => document.visibilityState),
    reader: await b.evaluate(() => document.visibilityState),
  };
  return {
    visibility,
    total: stats(rows.map((r) => r.total)),
    pause: stats(rows.map((r) => r.pause)),
    writeCall: stats(rows.map((r) => r.write)),
    fromWriteToOther: stats(rows.map((r) => r.fromWriteToOther)),
  };
};

/** A types for about 6 s without a pause; how long each character takes to reach B. */
const continuous = async (a, b) => {
  await a.bringToFront();
  await b.evaluate(() => window.spike.logLengths());
  await intoEditor(a);
  const startLength = await a.evaluate(() => window.spike.textLength());
  await a.evaluate(() => window.spike.resetKeys());
  await a.keyboard.type(" " + "steady typing ".repeat(4).slice(0, 59), {
    delay: 100,
  });
  await sleep(2500);
  const keys = await a.evaluate(() => window.spike.keyTimes());
  const grew =
    (await b.evaluate(() => window.spike.textLength())) - startLength;
  const lengths = await b.evaluate(() => window.spike.lengths());
  const lat = keys.map((k, i) => {
    const target = startLength + i + 1;
    const hit = lengths.find(([, len]) => len >= target);
    return hit ? hit[0] - k : null;
  });
  const writes = (await a.evaluate(() => window.spike.writes())).filter(
    (w) => w.lastAt >= keys[0],
  );
  return {
    keysRecorded: keys.length,
    charactersArrived: grew,
    perCharacter: stats(lat.filter((x) => x !== null)),
    missing: lat.filter((x) => x === null).length,
    writes: writes.map((w) => ({
      reason: w.reason,
      parts: w.parts,
      bytes: w.bytes,
    })),
  };
};

/** Prepares A and B on one space: A writes a first line, B opens once it holds it. */
const bind = async (a, b, spaceId) => {
  await a.evaluate(
    (id) => window.spike.setupSdk({ spaceId: id, clientID: 2 }),
    spaceId,
  );
  await intoEditor(a);
  await a.keyboard.type("Start.");
  await waitFor("B's database holds A's first write", () =>
    b.evaluate(async (id) => {
      const { db, model } = window.tab;
      const space = await db.spaces.get(id).catch(() => null);
      if (!space) return false;
      const page = await space.find(model.tasks.indexes.open.equal("open"));
      return page.items.length > 0;
    }, spaceId),
  );
  await b.evaluate(
    (id) => window.spike.setupSdk({ spaceId: id, clientID: 3 }),
    spaceId,
  );
};

const twoTabs = async (browser, url, trials) => {
  const context = await browser.newContext();
  const { page: a, problems: pa } = await openPage(context, url);
  const { page: b, problems: pb } = await openPage(context, url);
  await openDb(a, "latency");
  await openDb(b, "latency");
  const spaceId = await a.evaluate(
    async () => (await window.tab.db.spaces.create()).id,
  );
  await bind(a, b, spaceId);
  const out = {
    bursts: await bursts(a, b, trials),
    continuous: await continuous(a, b),
  };
  // A types and its tab is closed at once, before the pause.
  for (const runBeforeUnload of [false, true]) {
    const { page: c } = await openPage(context, url);
    await openDb(c, "latency");
    await c.evaluate(
      (id) => window.spike.setupSdk({ spaceId: id, clientID: 4 }),
      spaceId,
    );
    await intoEditor(c);
    const token = `closed${runBeforeUnload ? "B" : "A"}`;
    await c.keyboard.type(` ${token}`);
    await c.close({ runBeforeUnload });
    const arrived = await b
      .evaluate(
        (t) =>
          Promise.race([
            window.spike.whenHolds(t),
            new Promise((r) => setTimeout(() => r(null), 5000)),
          ]),
        token,
      )
      .then((x) => x !== null);
    const startedBy = await b.evaluate(() =>
      localStorage.getItem("spike.flushStart"),
    );
    out[`tab closed before the pause, runBeforeUnload ${runBeforeUnload}`] = {
      arrived,
      startedBy,
    };
    await b.evaluate(() => localStorage.removeItem("spike.flushStart"));
  }
  out.problems = [...pa, ...pb];
  await context.close();
  return out;
};

const twoBrowsers = async (url, trials) => {
  const { startRelay } = await import(
    join(ROOT, "scripts", "sdk-browser", "relay.mjs")
  );
  const browserA = await launch();
  const browserB = await launch();
  const { page: a, problems: pa } = await openPage(
    await browserA.newContext(),
    url,
  );
  const { page: b, problems: pb } = await openPage(
    await browserB.newContext(),
    url,
  );
  await openDb(a, "a");
  const invitationA = await a.evaluate(async () => {
    const text = String(await window.tab.db.invitation());
    await window.tab.db.close();
    return text;
  });
  const relay = await startRelay(["--allow-invitation", invitationA]);
  try {
    await openDb(a, "a", relay.url);
    await openDb(b, "b", relay.url);
    const invited = await b.evaluate(async () => {
      const invitation = await window.tab.db.invitation();
      return { text: String(invitation), fingerprint: invitation.fingerprint };
    });
    const spaceId = await a.evaluate(async ({ text, fingerprint }) => {
      const { db } = window.tab;
      const space = await db.spaces.create();
      await space.members.add(window.nc.invitation(text), {
        role: "writer",
        fingerprint,
      });
      return space.id;
    }, invited);
    await waitFor("B holds the space", () =>
      b.evaluate(
        (id) =>
          window.tab.db.spaces.get(id).then(
            () => true,
            () => false,
          ),
        spaceId,
      ),
    );
    await bind(a, b, spaceId);
    return {
      bursts: await bursts(a, b, trials),
      continuous: await continuous(a, b),
      problems: [...pa, ...pb],
    };
  } finally {
    await browserA.close();
    await browserB.close();
    relay.stop();
  }
};

export const runLatency = async (browser, url, { trials = 10 } = {}) => ({
  "two tabs of one browser": await twoTabs(browser, url, trials),
  "two browsers through the local relay": await twoBrowsers(url, trials),
});

// How the binding avoids echoing its own writes, and what happens without
// the check. One page, the in-page core, the older app's editor (client 2)
// and a collaborator without an editor (client 3).
import { openPage, sleep } from "./browser.mjs";

const fresh = async (context, url, o) => {
  const p = await openPage(context, url);
  await p.page.evaluate((o) => window.spike.setupLocal(o), o);
  return p;
};
const report = (page) => page.evaluate(() => window.spike.report());

// The person types in two bursts; each write comes back from the core.
const ownWrites = async (context, url) => {
  const { page, problems } = await fresh(context, url, { collaborator: true });
  await page.evaluate(() => window.spike.place("", { before: true }));
  await page.keyboard.type("Hello", { delay: 40 });
  await sleep(800);
  await page.keyboard.type(" world", { delay: 40 });
  await sleep(800);
  const r = await report(page);
  await page.close();
  return {
    writes: r.stats.writes,
    pullsAfterOwnWrites: r.stats.pulls,
    localUpdates: r.stats.localUpdates,
    problems,
  };
};

// The collaborator writes three times; the person does not type.
const othersWrites = async (context, url, filterOwnOrigin) => {
  const { page, problems } = await fresh(context, url, {
    collaborator: true,
    filterOwnOrigin,
  });
  await page.evaluate(() => window.spike.place("", { before: true }));
  await page.keyboard.type("Mine.", { delay: 40 });
  await sleep(800);
  for (const t of [" A", " B", " C"]) {
    await page.evaluate((t) => window.spike.collaboratorTypes(t), t);
    await sleep(100);
  }
  await sleep(1500);
  const r = await report(page);
  await page.close();
  return {
    filterOwnOrigin,
    editorText: r.editorText,
    writesOfTheBinding: r.stats.writes,
    echoedUpdates: r.stats.echoed,
    coreWrites: r.coreWrites,
    problems,
  };
};

// A pull applies Y.encodeStateAsUpdate(core, stateVector): new items, and the
// core's whole delete set. Its size after the binding's own write, as the
// person deletes more and more scattered characters.
const deleteSetInPulls = async (context, url) => {
  const { page, problems } = await fresh(context, url, {});
  await page.evaluate(() => window.spike.place("", { before: true }));
  await page.keyboard.type("abcdefghij".repeat(20));
  await sleep(700);
  const sizes = [];
  for (let round = 0; round < 4; round += 1) {
    // 25 scattered single-character deletions per round.
    await page.evaluate(() => {
      const { editor } = window.spike.editor();
      for (let i = 0; i < 25; i += 1) {
        const at = 2 + ((i * 7) % (editor.state.doc.content.size - 4));
        editor.commands.deleteRange({ from: at, to: at + 1 });
      }
    });
    await sleep(700);
    const r = await report(page);
    const pull = r.stats.pulls.at(-1);
    sizes.push({
      deletions: (round + 1) * 25,
      pullBytes: pull.bytes,
      changed: pull.changed,
    });
  }
  await page.close();
  return { sizes, problems };
};

// Undo with a collaborator: the person's undo takes back only the person's text.
const undoWithCollaborator = async (context, url) => {
  const { page, problems } = await fresh(context, url, { collaborator: true });
  await page.evaluate(() => window.spike.place("", { before: true }));
  await page.keyboard.type("Mine.", { delay: 40 });
  await sleep(800);
  await page.evaluate(() => window.spike.collaboratorTypes(" Theirs."));
  await sleep(200);
  await page.keyboard.press("Control+z");
  await sleep(800);
  const afterUndo = await report(page);
  await page.keyboard.press("Control+Shift+z");
  await sleep(800);
  const afterRedo = await report(page);
  await page.close();
  return {
    afterUndo: { editorText: afterUndo.editorText, core: afterUndo.core },
    afterRedo: { editorText: afterRedo.editorText, core: afterRedo.core },
    writes: afterRedo.stats.writes,
    problems,
  };
};

// Hiding the page writes at once, without waiting for the pause.
const hidden = async (context, url) => {
  const { page, problems } = await fresh(context, url, {});
  await page.evaluate(() => window.spike.place("", { before: true }));
  await page.keyboard.type("Bye", { delay: 40 });
  const before = (await report(page)).stats.writes.length;
  await page.evaluate(() => window.spike.hide());
  await sleep(50);
  const r = await report(page);
  await page.close();
  return {
    writesBeforeHide: before,
    writesAfterHide: r.stats.writes,
    problems,
  };
};

export const runEcho = async (browser, url) => {
  const context = await browser.newContext();
  const out = {
    "own writes come back": await ownWrites(context, url),
    "a collaborator writes, origin checked": await othersWrites(
      context,
      url,
      true,
    ),
    "a collaborator writes, origin not checked": await othersWrites(
      context,
      url,
      false,
    ),
    "the delete set in every pull": await deleteSetInPulls(context, url),
    "undo with a collaborator": await undoWithCollaborator(context, url),
    "hiding the page": await hidden(context, url),
  };
  await context.close();
  return out;
};

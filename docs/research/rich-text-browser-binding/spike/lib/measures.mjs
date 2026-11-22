// The three measures (widening, content check with rebuild, guard) in a real
// editor, with IME input, paste and undo, which F19 did not run. Each case
// opens the older app's editor on the newer app's document in a fresh page.
// "lost" lists what of the newer app's document the core no longer holds;
// "flagged" what the guard refused; a deliberate deletion is named as such.
import { compose, openPage, sleep } from "./browser.mjs";

const ALL = { newerDocument: true };

const open = async (context, url, o = {}) => {
  const p = await openPage(context, url);
  await p.page.evaluate((o) => window.spike.setupLocal(o), { ...ALL, ...o });
  return p;
};

const pause = () => sleep(700);

const outcome = async (page, problems, extra = {}) => {
  const r = await page.evaluate(() => window.spike.report());
  await page.close();
  return {
    ...extra,
    editorText: r.editorText,
    core: r.core,
    lost: r.lostInCore,
    flagged: r.stats.refused.flatMap((x) => x.flagged),
    reloads: r.stats.reloads,
    contentErrors: r.contentErrors,
    rebuilds: r.rebuilds,
    writes: r.stats.writes.length,
    editable: r.editable,
    problems,
  };
};

const core = (page) => page.evaluate(() => window.spike.report().core);

// IME: a composition committed in a paragraph whose `align` the older app
// does not know, held for 1.5 s before it commits, longer than the pause.
const imeInAlignedParagraph = async (context, url) => {
  const { page, problems } = await open(context, url);
  await page.evaluate(() => window.spike.place("Centered."));
  const steps = ["に", "にほ", "にほん"];
  const writesBefore = (await page.evaluate(() => window.spike.writes()))
    .length;
  const cdp = await context.newCDPSession(page);
  for (const text of steps) {
    await cdp.send("Input.imeSetComposition", {
      text,
      selectionStart: text.length,
      selectionEnd: text.length,
    });
    await sleep(120);
  }
  await sleep(1500);
  const duringComposition = {
    writes:
      (await page.evaluate(() => window.spike.writes())).length - writesBefore,
    core: await core(page),
  };
  await cdp.send("Input.insertText", { text: "日本" });
  await cdp.detach();
  await pause();
  return outcome(page, problems, { duringComposition });
};

// IME right after the inline placeholder `chip`, and at the end of the text
// the unknown mark `highlight` covers.
const imeNextToPlaceholders = async (context, url) => {
  const { page, problems } = await open(context, url);
  await page.evaluate(() => window.spike.place(" here.", { before: true }));
  await compose(page, ["か", "かん"], "漢");
  await page.evaluate(() => window.spike.place("marked"));
  await compose(page, ["じ"], "字");
  await pause();
  return outcome(page, problems);
};

// IME under way when a kind of node the older app has never seen arrives:
// contentError, and the editor is rebuilt in the middle of the composition.
const imeWhileNewKindArrives = async (context, url) => {
  const { page, problems } = await open(context, url);
  await page.evaluate(() => window.spike.place("Intro."));
  const cdp = await context.newCDPSession(page);
  await cdp.send("Input.imeSetComposition", {
    text: "とう",
    selectionStart: 2,
    selectionEnd: 2,
  });
  await sleep(100);
  await page.evaluate(() => window.spike.addFigure());
  await sleep(200);
  const focused = await page.evaluate(
    () => window.spike.editor().editor.isFocused,
  );
  await cdp.send("Input.imeSetComposition", {
    text: "とうきょう",
    selectionStart: 5,
    selectionEnd: 5,
  });
  await sleep(100);
  await cdp.send("Input.insertText", { text: "東京" });
  await cdp.detach();
  await pause();
  // The person clicks back into the editor and types on.
  await page.evaluate(() => window.spike.place("Intro."));
  await page.keyboard.type(" ok");
  await pause();
  return outcome(page, problems, { focusedAfterRebuild: focused });
};

// Undo of a composition, in the aligned paragraph.
const undoIme = async (context, url) => {
  const { page, problems } = await open(context, url);
  await page.evaluate(() => window.spike.place("Centered."));
  await compose(page, ["に", "にほ", "にほん"], "日本");
  await pause();
  await page.keyboard.press("Control+z");
  await pause();
  return outcome(page, problems);
};

// Paste plain text over a selection that spans the chip: a deliberate
// deletion of a placeholder, which the guard lets through.
const pasteOverPlaceholder = async (context, url) => {
  const { page, problems } = await open(context, url);
  await page.evaluate(() => window.spike.selectBetween("See", "here."));
  await page.evaluate(() => window.spike.paste({ text: "Replaced." }));
  await pause();
  return outcome(page, problems, { deliberatelyDeleted: ["chip"] });
};

// Copy a placeholder and paste it elsewhere, with and without parse rules.
const copyPastePlaceholder = async (context, url, node, roundTrip) => {
  const { page, problems } = await open(context, url, { roundTrip });
  await page.evaluate((n) => window.spike.selectNode(n), node);
  const clip = await page.evaluate(() => window.spike.copySelection());
  await page.evaluate(() => window.spike.place("Intro."));
  await page.evaluate((c) => window.spike.paste(c), clip);
  await pause();
  const c = await core(page);
  const count = (c.match(new RegExp(`<${node}[ >]`, "g")) ?? []).length;
  return outcome(page, problems, {
    roundTrip,
    clipboardHtml: clip.html,
    copiesInCore: count,
  });
};

// Copy the highlighted word, whose mark the older app does not know.
const copyPasteUnknownMark = async (context, url) => {
  const { page, problems } = await open(context, url);
  await page.evaluate(() => window.spike.selectBetween("marked", "marked"));
  const clip = await page.evaluate(() => window.spike.copySelection());
  await page.evaluate(() => window.spike.place("Intro."));
  await page.evaluate((c) => window.spike.paste(c), clip);
  await pause();
  const c = await core(page);
  return outcome(page, problems, {
    clipboardHtml: clip.html,
    highlightsInCore: (c.match(/<highlight/g) ?? []).length,
  });
};

// HTML from another app, with tags the older app has no rule for.
const pasteForeignHtml = async (context, url) => {
  const { page, problems } = await open(context, url);
  await page.evaluate(() => window.spike.place("Intro."));
  await page.evaluate(() =>
    window.spike.paste({
      html: '<aside class="callout">Pasted aside</aside><p><mark>pasted mark</mark></p>',
      text: "Pasted aside\npasted mark",
    }),
  );
  await pause();
  return outcome(page, problems);
};

// Typing in the aligned paragraph, then undo and redo: is `align` kept?
const undoInAlignedParagraph = async (context, url) => {
  const { page, problems } = await open(context, url);
  await page.evaluate(() => window.spike.place("Centered."));
  await page.keyboard.type(" Now", { delay: 30 });
  await pause();
  await page.keyboard.press("Control+z");
  await pause();
  await page.keyboard.press("Control+Shift+z");
  await pause();
  return outcome(page, problems);
};

// Delete a placeholder on purpose, then undo: does it come back with its attributes?
const undoPlaceholderDeletion = async (context, url, node) => {
  const { page, problems } = await open(context, url);
  await page.evaluate((n) => window.spike.selectNode(n), node);
  await page.keyboard.press("Backspace");
  await pause();
  const afterDelete = await core(page);
  await page.keyboard.press("Control+z");
  await pause();
  return outcome(page, problems, {
    node,
    deletedInCore: !afterDelete.includes(`<${node}`),
  });
};

// Undo after a rebuild, with the editor's own UndoManager and with one the
// binding keeps across rebuilds.
const undoAcrossRebuild = async (context, url, keepUndo) => {
  const { page, problems } = await open(context, url, { keepUndo });
  await page.evaluate(() => window.spike.place("Intro."));
  await page.keyboard.type(" Typed before.", { delay: 20 });
  await pause();
  await page.evaluate(() => window.spike.addFigure());
  await sleep(300);
  await page.evaluate(() => window.spike.place("Intro."));
  const undid = await page.evaluate(() => window.spike.undo());
  await pause();
  return outcome(page, problems, { keepUndo, undoCommandRan: undid });
};

// A new kind arrives, and the app listens for contentError as RFC 0013's
// example does, with editor.on, instead of the editor's onContentError option.
const lateArrival = async (context, url, errorVia) => {
  const { page, problems } = await open(context, url, { errorVia });
  await page.evaluate(() => window.spike.addFigure());
  await sleep(300);
  await page.evaluate(() => window.spike.place("Intro."));
  await page.keyboard.type(" after");
  await pause();
  return outcome(page, problems, { errorVia });
};

// The content check paused, no rebuild: typing and pasting are refused.
const pausedEditor = async (context, url) => {
  const { page, problems } = await open(context, url, {
    rebuildOnError: false,
  });
  await page.evaluate(() => window.spike.addFigure());
  await sleep(200);
  await page.evaluate(() => window.spike.place("Intro."));
  await page.keyboard.type(" typed");
  await page.evaluate(() => window.spike.paste({ text: " pasted" }));
  await compose(page, ["に"], "日");
  await pause();
  return outcome(page, problems);
};

// The guard alone: no widening and no content check. The editor deletes the
// unknown parts as it opens; the guard refuses the write and reloads, which
// rebuilds the editor, which deletes again.
const guardAlone = async (context, url) => {
  const { page, problems } = await open(context, url, {
    widening: false,
    contentCheck: false,
  });
  await sleep(3000);
  const r = await outcome(page, problems, { watchedMs: 3000 });
  return { ...r, flagged: [...new Set(r.flagged)] };
};

// The same, and the person types in the first paragraph meanwhile: the
// guard refuses the whole pending write, the person's typing with it.
const guardAloneTyping = async (context, url) => {
  const { page, problems } = await open(context, url, {
    widening: false,
    contentCheck: false,
  });
  await sleep(100);
  await page.evaluate(() => window.spike.place("Intro."));
  await page.keyboard.type(" Typed.", { delay: 30 });
  await sleep(1500);
  const r = await outcome(page, problems);
  return {
    ...r,
    flagged: [...new Set(r.flagged)],
    typedKept: r.core.includes("Typed."),
  };
};

export const runMeasures = async (browser, url) => {
  const context = await browser.newContext();
  const out = {
    "IME in the aligned paragraph, held past the pause":
      await imeInAlignedParagraph(context, url),
    "IME next to the chip and inside the highlight":
      await imeNextToPlaceholders(context, url),
    "IME while a new kind arrives": await imeWhileNewKindArrives(context, url),
    "undo of a composition": await undoIme(context, url),
    "paste over a selection spanning the chip": await pasteOverPlaceholder(
      context,
      url,
    ),
    "copy and paste the chip, with parse rules": await copyPastePlaceholder(
      context,
      url,
      "chip",
      true,
    ),
    "copy and paste the chip, without parse rules": await copyPastePlaceholder(
      context,
      url,
      "chip",
      false,
    ),
    "copy and paste the callout, with parse rules": await copyPastePlaceholder(
      context,
      url,
      "callout",
      true,
    ),
    "copy and paste the highlighted word": await copyPasteUnknownMark(
      context,
      url,
    ),
    "paste HTML from another app": await pasteForeignHtml(context, url),
    "undo and redo in the aligned paragraph": await undoInAlignedParagraph(
      context,
      url,
    ),
    "delete the chip, then undo": await undoPlaceholderDeletion(
      context,
      url,
      "chip",
    ),
    "delete the callout, then undo": await undoPlaceholderDeletion(
      context,
      url,
      "callout",
    ),
    "undo across a rebuild, editor's own UndoManager": await undoAcrossRebuild(
      context,
      url,
      false,
    ),
    "undo across a rebuild, UndoManager kept by the binding":
      await undoAcrossRebuild(context, url, true),
    "a new kind arrives, rebuild from the onContentError option":
      await lateArrival(context, url, "option"),
    "a new kind arrives, rebuild from editor.on as in RFC 0013":
      await lateArrival(context, url, "listener"),
    "content check paused, no rebuild": await pausedEditor(context, url),
    "guard alone, no widening, no content check": await guardAlone(
      context,
      url,
    ),
    "guard alone, and the person types": await guardAloneTyping(context, url),
  };
  await context.close();
  return out;
};

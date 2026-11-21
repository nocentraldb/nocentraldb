// NOC-436 spike: how an agent's edit of a Yjs/ProseMirror document behaves
// while a person keeps typing. Run: npm install --no-package-lock && node spike.mjs
// Every experiment prints what it observed; the note quotes these lines.
import { defaultMarkdownSerializer } from "prosemirror-markdown";
import {
  absolutePositionToRelativePosition,
  relativePositionToAbsolutePosition,
} from "y-prosemirror";
import {
  authors,
  blockText,
  editThroughProseMirror,
  frag,
  insertText,
  newMember,
  paragraph,
  pmDoc,
  quoteEditAtRead,
  quoteEditNow,
  readVersion,
  report,
  schema,
  seed,
  sync,
  text,
  Y,
} from "./lib.mjs";

const PERSON = 1;
const AGENT = 2;
const FOLD = 20; // the agent member's folded copy, which writes nothing itself
const START = ["Intro.", "We ship on Friday.", "Outro."];

const pair = (agentCopy = AGENT) => {
  const person = newMember(PERSON);
  const agent = newMember(agentCopy);
  seed(person, START);
  sync(person, agent);
  return { person, agent };
};

// S1. A ProseMirror position the agent got at read time goes stale when the
// person types before it; a Yjs relative position does not.
{
  console.log("\nS1: plain position vs relative position");
  const { person, agent } = pair();
  const { doc: pm, mapping } = pmDoc(agent);
  let pos = -1;
  pm.descendants((n, p) => {
    if (n.isText && n.text.includes("Friday"))
      pos = p + n.text.indexOf("Friday");
  });
  const rel = absolutePositionToRelativePosition(pos, frag(agent), mapping);
  blockText(person, 1).insert(0, "Good news: ");
  sync(person, agent);
  const now = pmDoc(agent);
  report("plain position now points at", now.doc.textBetween(pos, pos + 6));
  const moved = relativePositionToAbsolutePosition(
    agent,
    frag(agent),
    rel,
    now.mapping,
  );
  report(
    "relative position now points at",
    now.doc.textBetween(moved, moved + 6),
  );
}

// S2. Resolve the quote at write time. The person typed elsewhere: fine. The
// person typed inside the quote: the anchor is gone and the agent must re-read.
{
  console.log("\nS2: quote resolved at write time");
  const { person, agent } = pair();
  blockText(person, 0).insert(5, " Hello");
  sync(person, agent);
  report(
    "person typed elsewhere, edit applied",
    String(quoteEditNow(agent, "Friday", "Monday")),
  );
  sync(person, agent);
  report("result", text(person));
  const b = pair();
  blockText(b.person, 1).insert(14, "-"); // "Fri-day"
  sync(b.person, b.agent);
  report(
    "person typed inside quote, edit applied",
    String(quoteEditNow(b.agent, "Friday", "Monday")),
  );
}

// S3. Edit against the version the agent read, in a fork, then merge. The
// person typed inside the quote after the read: both edits survive.
{
  console.log("\nS3: quote resolved against the read version (fork and merge)");
  const { person, agent } = pair(FOLD);
  const read = readVersion(agent);
  blockText(person, 1).insert(14, "-"); // "Fri-day"
  blockText(person, 1).insert(18, " (tentative)");
  sync(person, agent);
  report("live text before the agent writes", text(agent));
  const update = quoteEditAtRead(agent, AGENT, read, "Friday", "Monday");
  sync(person, agent);
  report("edit applied", String(update !== null));
  report("update bytes", String(update.length));
  report(
    "result on both",
    text(person) === text(agent) ? text(person) : "DIVERGED",
  );
  report("who wrote block 1", authors(person, 1));
}

// S4. Truly simultaneous: neither has seen the other's edit when it writes.
// A focused edit keeps the person's typing; replacing the paragraph loses it.
{
  console.log("\nS4: simultaneous edits, focused vs whole paragraph");
  const { person, agent } = pair();
  blockText(person, 1).insert(18, " Bob typed this.");
  quoteEditNow(agent, "Friday", "Monday");
  sync(person, agent);
  report("focused edit", text(person));
  const b = pair();
  blockText(b.person, 1).insert(18, " Bob typed this.");
  frag(b.agent).delete(1, 1);
  frag(b.agent).insert(1, [paragraph("We ship on Monday.")]);
  sync(b.person, b.agent);
  report("whole paragraph replaced", text(b.person));
  const c = pair();
  blockText(c.person, 1).insert(18, " Bob typed this.");
  const t = blockText(c.agent, 1);
  t.delete(0, t.length);
  t.insert(0, "We ship on Monday.");
  sync(c.person, c.agent);
  report("whole text of the paragraph replaced", text(c.person));
}

// S5. Streaming: the agent types a sentence word by word at the end of
// block 1 while the person types at the same spot. Each side keeps its own
// insertion point as a relative position (assoc -1: sticks to the character
// before it, which is its own last character).
{
  console.log(
    "\nS5: agent streams words while the person types at the same place",
  );
  const { person, agent } = pair();
  const end = (doc) =>
    Y.createRelativePositionFromTypeIndex(
      blockText(doc, 1),
      blockText(doc, 1).length,
      -1,
    );
  let agentAt = end(agent);
  let personAt = end(person);
  const agentWords = [" The", " agent", " adds", " this."];
  const personWords = [" I", " type", " here", " too."];
  for (let i = 0; i < 4; i++) {
    for (const [doc, words, at, set] of [
      [agent, agentWords, agentAt, (r) => (agentAt = r)],
      [person, personWords, personAt, (r) => (personAt = r)],
    ]) {
      const t = blockText(doc, 1);
      const idx = Y.createAbsolutePositionFromRelativePosition(at, doc).index;
      t.insert(idx, words[i]);
      set(Y.createRelativePositionFromTypeIndex(t, idx + words[i].length, -1));
    }
    sync(person, agent);
  }
  report("result", text(person));
  report("who wrote block 1", authors(person, 1));
}

// S6. The agent writes twice from one read. A fork that leaves out the
// agent's own first edit reuses its clocks: the same item id with two
// contents, which RFC 0013 F6 shows diverging. A fork that adds the agent's
// own updates since the read continues the clock.
{
  console.log("\nS6: two edits from one read, and the agent's clock");
  const { person, agent } = pair(FOLD);
  const read = readVersion(agent);
  blockText(person, 0).insert(0, "Hi. ");
  sync(person, agent);
  const first = quoteEditAtRead(agent, AGENT, read, "Friday", "Monday");
  const second = quoteEditAtRead(
    agent,
    AGENT,
    read,
    "Outro.",
    "Outro, revised.",
    [first],
  );
  sync(person, agent);
  report(
    "with own updates in the fork",
    text(person) === text(agent) ? text(person) : "DIVERGED",
  );
  report("second update's client and first clock", [
    Y.decodeUpdate(second).structs[0].id.client,
    Y.decodeUpdate(second).structs[0].id.clock,
  ]);
  const b = pair(FOLD);
  const readB = readVersion(b.agent);
  const firstB = quoteEditAtRead(b.agent, AGENT, readB, "Friday", "Monday");
  const fork = newMember(AGENT);
  Y.applyUpdate(fork, readB);
  quoteEditNow(fork, "Outro.", "Outro, revised.");
  const reused = Y.encodeStateAsUpdate(fork, Y.encodeStateVector(newMember(9)));
  const firstClock = Y.decodeUpdate(firstB).structs[0].id.clock;
  const forkClock = Y.decodeUpdate(reused).structs.find(
    (s) => s.id.client === AGENT,
  ).id.clock;
  report("first edit's first clock", String(firstClock));
  report("forgetful fork's first clock", String(forkClock));
  const x = newMember(31);
  const y = newMember(32);
  for (const [doc, order] of [
    [x, [readB, firstB, reused]],
    [y, [readB, reused, firstB]],
  ]) {
    for (const u of order) Y.applyUpdate(doc, u);
  }
  report("copy that got the first edit first", text(x));
  report("copy that got the forgetful edit first", text(y));
}

// S7. Markdown as the quote: what the agent reads is not always what it
// would type. Escapes, a mark that covers part of a word, and marks that
// Markdown cannot show.
{
  console.log("\nS7: Markdown quotes");
  const strong = schema.marks.strong.create();
  const pm = schema.node("doc", null, [
    schema.node("paragraph", null, [schema.text("Price: 5 * 3 = 15 [draft]")]),
    schema.node("paragraph", null, [
      schema.text("We ship on "),
      schema.text("Fri", [strong]),
      schema.text("day."),
    ]),
    schema.node("paragraph", null, [schema.text("Ship it.")]),
    schema.node("paragraph", null, [schema.text("Ship it.")]),
  ]);
  const md = defaultMarkdownSerializer.serialize(pm);
  for (const line of md.split("\n\n")) report("markdown", line);
  report("quote '5 * 3' found in markdown", String(md.includes("5 * 3")));
  report("quote 'Friday' found in markdown", String(md.includes("Friday")));
  report(
    "quote 'Friday' found in text",
    String(pm.textContent.includes("Friday")),
  );
  report("quote 'Ship it.' matches", String(md.split("Ship it.").length - 1));
}

// S8. Several small agent writes as the agent "types": the cost per write in
// Yjs bytes, which RFC 0013 D3 then wraps in a change each.
{
  console.log("\nS8: size of a typed word vs a focused replacement");
  const { agent } = pair();
  const sizes = [];
  agent.on("update", (u) => sizes.push(u.length));
  editThroughProseMirror(agent, (tr, pm) => {
    let at = -1;
    pm.descendants((n, p) => {
      if (n.isText && n.text.includes("Friday"))
        at = p + n.text.indexOf("Friday") + 6;
    });
    insertText(tr, " afternoon", at);
  });
  quoteEditNow(agent, "Friday afternoon", "Monday morning");
  report("insert ' afternoon' (bytes)", String(sizes[0]));
  report("'Friday afternoon' -> 'Monday morning' (bytes)", String(sizes[1]));
  report("result", text(agent));
}

// S9. Marks and block types through ProseMirror steps, while the person types
// in the same paragraph. A mark is an attribute on a text range; a new block
// type is a new element, which y-prosemirror writes as delete and insert.
{
  console.log("\nS9: a mark and a block type while the person types");
  const { person, agent } = pair();
  blockText(person, 1).insert(18, " Bob typed this.");
  editThroughProseMirror(agent, (tr, pm) => {
    let at = -1;
    pm.descendants((n, p) => {
      if (n.isText && n.text.includes("Friday"))
        at = p + n.text.indexOf("Friday");
    });
    tr.addMark(at, at + 6, schema.marks.strong.create());
  });
  sync(person, agent);
  report("bold on 'Friday'", text(person));
  report("block 1 as Yjs holds it", blockText(person, 1).toDelta());
  const b = pair();
  blockText(b.person, 1).insert(18, " Bob typed this.");
  editThroughProseMirror(b.agent, (tr, pm) => {
    const start = pm.child(0).nodeSize;
    tr.setBlockType(start + 1, start + 1, schema.nodes.heading, { level: 2 });
  });
  sync(b.person, b.agent);
  report("paragraph made a heading", text(b.person));
  report("block 1's element", frag(b.person).get(1).nodeName);
}

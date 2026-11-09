// Asserts that the documents follow the owner's rulings on RFC 0007's
// "Conflicts" section (NOC-47): RFC 0002 carries Amendment 4, which a dated
// changelog line at the top announces and whose every point links RFC 0007;
// RFC 0002's list of what the relay sees links RFC 0007's, and so does every
// text that links RFC 0002's list; RFC 0002 states the uploader rule; use
// case 4 promises only reads at no cost; use case 5 says how peers find each
// other is still missing; and RFC 0007 marks those conflicts resolved. The
// test checks that each claim is there and points to its source; whether the
// wording is right is the owner's review of the PR.
import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

const RFC_0001 = "docs/rfc/0001-nocentraldb.md";
const RFC_0002 = "docs/rfc/0002-change-and-log-format.md";
const RFC_0007 = "docs/rfc/0007-relay.md";
const FAQ = "docs/faq.md";
const USE_CASES = "docs/use-cases.md";
const GENESIS = "factory/GENESIS.md";
const DECISIONS = "factory/decisions";

const AMENDMENT = "amendment-4-following-rfc-0007";
const RFC_0002_LIST =
  "0002-change-and-log-format.md#what-the-relay-can-see-and-what-it-cannot";
const RFC_0007_SEES = "0007-relay.md#what-the-relay-sees";

/** GitHub's heading anchor: lower case, punctuation dropped, spaces to dashes. */
const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .replace(/\s/g, "-");
const anchors = (markdown) =>
  new Set(
    markdown
      .split("\n")
      .filter((line) => /^#{1,6} /.test(line))
      .map((line) => slug(line.replace(/^#+ /, ""))),
  );

/** The text under the heading whose anchor is `anchor`, up to the next heading of the same or a higher level. */
const section = (markdown, anchor) => {
  const lines = markdown.split("\n");
  const start = lines.findIndex(
    (line) =>
      /^#{1,6} /.test(line) && slug(line.replace(/^#+ /, "")) === anchor,
  );
  if (start < 0) return undefined;
  const level = lines[start].match(/^#+/)[0].length;
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((line) => {
    const m = line.match(/^(#+) /);
    return m && m[1].length <= level;
  });
  return (end < 0 ? rest : rest.slice(0, end)).join("\n");
};

/** The Markdown link targets in `text`. */
const targets = (text) =>
  [...text.matchAll(/\]\(([^)\s]+)\)/g)].map((m) => m[1]);
const links = (text, target) => targets(text).some((t) => t.endsWith(target));

/** True if `target`, relative to `from`, names a heading of RFC 0007 that exists. */
const resolvesInto0007 = (from, target) => {
  const [file, anchor] = target.split("#");
  if (normalize(join(dirname(from), file)) !== RFC_0007 || !anchor) {
    return false;
  }
  return anchors(read(RFC_0007)).has(anchor);
};

test("RFC 0002 announces Amendment 4 in a dated NOC-47 changelog line that links RFC 0007", () => {
  const top = read(RFC_0002).split(/^## /m)[0];
  const line = top
    .split("\n")
    .find((l) => /^- \*\*\d{4}-\d{2}-\d{2}\*\* · NOC-47 · /.test(l));
  assert.ok(line, "no dated changelog line for NOC-47 above the first section");
  assert.ok(
    links(line, `#${AMENDMENT}`),
    `the changelog line does not link #${AMENDMENT}`,
  );
  assert.ok(
    targets(line).some((t) => t.split("#")[0] === "0007-relay.md"),
    "the changelog line does not link RFC 0007",
  );
});

test("RFC 0002's status says Amendment 4 decides nothing and follows RFC 0007", () => {
  const status = read(RFC_0002)
    .split("\n")
    .find((l) => l.startsWith("**Status.**"));
  assert.ok(links(status, `#${AMENDMENT}`), "the status does not name it");
  assert.match(status, /NOC-47/);
});

// RFC 0001's relay bullet no longer calls RFC 0002's list complete, which
// changes what RFC 0001 claims, so RFC 0001 records it as an amendment of
// its own, the way NOC-44 did with its Amendment 1.
const RFC_0001_AMENDMENT = "amendment-2-following-rfc-0007";

test("RFC 0001 announces Amendment 2 in a dated NOC-47 changelog line that links RFC 0007", () => {
  const top = read(RFC_0001).split(/^## /m)[0];
  const line = top
    .split("\n")
    .find((l) => /^- \*\*\d{4}-\d{2}-\d{2}\*\* · NOC-47 · /.test(l));
  assert.ok(line, "no dated changelog line for NOC-47 above the first section");
  assert.ok(
    links(line, `#${RFC_0001_AMENDMENT}`),
    `the changelog line does not link #${RFC_0001_AMENDMENT}`,
  );
  assert.ok(
    targets(line).some((t) => t.split("#")[0] === "0007-relay.md"),
    "the changelog line does not link RFC 0007",
  );
});

test("RFC 0001 has the section of Amendment 2: it decides nothing and links RFC 0007's list", () => {
  const rfc = read(RFC_0001);
  const body = section(rfc, RFC_0001_AMENDMENT);
  assert.ok(body, `no section #${RFC_0001_AMENDMENT}`);
  assert.match(body, /NOC-47/);
  assert.match(body, /no (new )?decision|decides nothing/i);
  assert.ok(links(body, RFC_0007_SEES), "no link to RFC 0007's list");
  const relay = rfc.split("\n").find((l) => l.startsWith("- **A relay"));
  assert.match(
    relay,
    /\*\(Amendment 2\.\)\*/,
    "the relay bullet's changed sentence is not marked",
  );
});

test("RFC 0002 has the section of Amendment 4, and every point links a heading of RFC 0007", () => {
  const body = section(read(RFC_0002), AMENDMENT);
  assert.ok(body, `no section #${AMENDMENT}`);
  const points = body.split("\n").filter((line) => line.startsWith("- **"));
  assert.ok(points.length >= 2, "the amendment lists fewer than two points");
  for (const point of points) {
    assert.ok(
      targets(point).some((t) => resolvesInto0007(RFC_0002, t)),
      `a point links no heading of RFC 0007: ${point.slice(0, 100)}`,
    );
  }
  assert.match(body, /uploader/i, "the amendment does not name the uploader");
  assert.ok(
    links(body, RFC_0007_SEES),
    "the amendment does not link RFC 0007's What the relay sees",
  );
});

test("RFC 0002's list of what the relay sees links RFC 0007's, marked Amendment 4", () => {
  const list = section(
    read(RFC_0002),
    "what-the-relay-can-see-and-what-it-cannot",
  );
  assert.ok(list, "no section on what the relay sees");
  const line = list.split("\n").find((l) => links(l, RFC_0007_SEES));
  assert.ok(line, "the list does not link RFC 0007's What the relay sees");
  assert.match(line, /server/i, "does not say RFC 0007 separates the server");
  assert.match(line, /\*\(Amendment 4\.\)\*/, "the sentence is not marked");
});

test("RFC 0002 says the uploader must be an authenticated current member, the author any member", () => {
  const relay = section(read(RFC_0002), "on-the-relay");
  assert.ok(relay, "no section 'On the relay'");
  const line = relay.split("\n").find((l) => /uploader/i.test(l));
  assert.ok(line, "no sentence on the uploader");
  assert.match(line, /authenticated current member/i);
  assert.match(
    line,
    /any member/i,
    "does not say the author may be any member",
  );
  assert.ok(
    links(line, "0007-relay.md#what-the-relay-checks"),
    "no link to RFC 0007's What the relay checks",
  );
  assert.match(line, /\*\(Amendment 4\.\)\*/, "the sentence is not marked");
  assert.ok(
    !/does not need to know who uploaded/.test(read(RFC_0002)),
    "RFC 0002 still says the relay need not know the uploader",
  );
});

// Where RFC 0001, use case 2 or the FAQ link RFC 0002's list (GENESIS.md
// links no RFC, as the owner decided on 2026-11-09),
// they link RFC 0007's too, and no longer say RFC 0002 lists all of it.
for (const [path, find] of [
  [RFC_0001, (t) => t.split("\n").find((l) => l.startsWith("- **A relay"))],
  [
    USE_CASES,
    (t) => section(t, "2-a-breach-of-the-relay-leaks-nothing-readable"),
  ],
  [FAQ, (t) => section(t, "what-can-the-relay-see")],
]) {
  test(`${path} links RFC 0007's What the relay sees next to RFC 0002's list`, () => {
    const text = find(read(path));
    assert.ok(text, "the passage on what the relay sees is missing");
    const line = text.split("\n").find((l) => links(l, RFC_0002_LIST));
    assert.ok(line, "the passage no longer links RFC 0002's list");
    assert.ok(links(line, RFC_0007_SEES), "no link to RFC 0007's list");
    assert.match(line, /server/i, "does not say RFC 0007 names the server");
    assert.ok(
      !/lists all of it/.test(line),
      "still says RFC 0002 lists all of it",
    );
  });
}

test("use case 4 promises reads at no cost, and says what writes cost", () => {
  const uc = read(USE_CASES);
  const anchor = "4-a-thousand-agent-queries-a-minute-reads-at-no-cost";
  const text = section(uc, anchor);
  assert.ok(text, `no section #${anchor}`);
  assert.ok(links(uc, `#${anchor}`), "the table of contents is not updated");
  const line = text
    .split("\n")
    .find((l) => links(l, "0007-relay.md#what-it-costs"));
  assert.ok(line, "no link to RFC 0007's What it costs");
  assert.match(line, /writes? (are|is) not free|cost/i);
  assert.match(line, /write/i, "does not say writes cost");
});

test("use case 5 says RFC 0007 defines the messages but not how peers find each other", () => {
  const text = section(
    read(USE_CASES),
    "5-the-relay-goes-down-and-everyone-keeps-working",
  );
  assert.ok(text, "no use case 5");
  const line = text
    .split("\n")
    .find(
      (l) =>
        /find each other/i.test(l) && links(l, "0007-relay.md#island-mode"),
    );
  assert.ok(
    line,
    "no passage on finding peers that links RFC 0007's Island mode",
  );
  assert.match(line, /messages/i, "does not say RFC 0007 defines the messages");
  assert.match(line, /peer RFC|RFC on peers/i, "does not name the peer RFC");
});

test("RFC 0007's Conflicts section says points 1 to 4 are resolved by NOC-47", () => {
  const conflicts = section(
    read(RFC_0007),
    "conflicts-with-genesismd-and-rfc-0001",
  );
  assert.ok(conflicts, "no Conflicts section");
  for (const n of [1, 2, 3, 4]) {
    const point = conflicts.split("\n").find((l) => l.startsWith(`${n}. `));
    assert.ok(point, `no conflict ${n}`);
    assert.match(point, /Resolved/, `conflict ${n} is not marked resolved`);
  }
  assert.ok(
    !/are follow-up ticket NOC-47/.test(conflicts),
    "the intro still calls the changes a follow-up",
  );
});

test("every relative link in the changed documents resolves, anchors included", () => {
  for (const path of [RFC_0001, RFC_0002, RFC_0007, FAQ, USE_CASES, GENESIS]) {
    const text = read(path).replace(/```[\s\S]*?```/g, "");
    for (const target of targets(text).filter((t) => !/^[a-z]+:/.test(t))) {
      const [file, anchor] = target.split("#");
      const resolved = file ? normalize(join(dirname(path), file)) : path;
      assert.ok(existsSync(join(root, resolved)), `${path}: ${target}`);
      if (!anchor || !resolved.endsWith(".md")) continue;
      assert.ok(
        anchors(read(resolved)).has(anchor),
        `${path}: ${target} names no heading`,
      );
    }
  }
});

test("the decision log records NOC-47's routing of the owner's RFC 0007 rulings", () => {
  const texts = readdirSync(join(root, DECISIONS))
    .filter((f) => f.includes("-NOC-47-"))
    .map((f) => read(join(DECISIONS, f)));
  assert.ok(
    texts.some((t) => /- Decided by: pm/.test(t) && /RFC 0007/.test(t)),
    "no decision by pm routing the owner's RFC 0007 rulings",
  );
});

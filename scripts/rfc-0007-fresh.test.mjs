// Holds RFC 0007 as a fresh text (NOC-165). The owner wants every RFC to read
// as if it had just been written (Carsten, 2026-11-10): each choice stands in
// the section it belongs to, with a sentence on why and the options not taken,
// and there is no list of decisions, rejected options, conflicts or open
// questions, no reference such as "R6", "M3" or "D12", and no History of the
// drafts. The tests below pin the content of every former decision R1 to R13,
// every row of the former "Rejected options" and every former conflict where
// it now stands, and the owner's answers written in with this rewrite:
// NOC-159, where TLS ends for a self-hosted relay and on AWS, and NOC-133
// point 2, a person-key record whose writer has since left. Whether the
// wording is right is the review of the PR.
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { join, normalize } from "node:path";
import { test } from "node:test";
import { headings, read, root, section, slug } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0007-relay.md";
const DISCUSSION = "https://github.com/nocentraldb/nocentraldb/discussions/147";
const PUBLISHED =
  "2026-11-10 · First published (RFC 0001 to RFC 0010 rewritten as fresh texts, NOC-165)";

const part = (anchor) => section(read(RFC), anchor);

const links = (text, target) =>
  [...text.matchAll(/\]\(([^)\s]+)\)/g)].some((m) => m[1].endsWith(target));

const line = (text, start) => {
  const found = text.split("\n").find((l) => l.startsWith(start));
  assert.ok(found, `no line starts with "${start}"`);
  return found;
};

const holds = (text, patterns, where) => {
  for (const pattern of patterns) {
    assert.match(text, pattern, `${where} does not say ${pattern}`);
  }
};

const documents = () => [
  "README.md",
  "CONTRIBUTING.md",
  ...readdirSync(join(root, "docs"), { recursive: true })
    .filter((file) => file.endsWith(".md"))
    .map((file) => normalize(join("docs", file))),
];

test("RFC 0007 has no list of decisions, rejected options, conflicts or open questions", () => {
  const found = headings(read(RFC)).map(slug);
  for (const gone of [
    "decisions",
    "rejected-options",
    "conflicts-with-genesismd-and-rfc-0001",
    "open-questions",
  ]) {
    assert.ok(!found.includes(gone), `the section #${gone} is left`);
  }
  assert.ok(
    !found.some((anchor) => /^r\d+-/.test(anchor)),
    "a decision heading such as R1 is left",
  );
});

test("RFC 0007 names no decision by its id, its own or another RFC's", () => {
  const text = read(RFC).replace(part("history"), "");
  for (const pattern of [
    /\bR\d+\b/,
    /\bM\d+\b/,
    /\bD\d+\b/,
    /\bS(?!3\b)\d+\b/, // S3 is the AWS service
    /\bdecisions? \d/i,
    /\bConflicts points?\b/,
  ]) {
    const hit = text.match(pattern);
    assert.ok(!hit, `RFC 0007 still names "${hit?.[0]}"`);
  }
});

test("no document links a removed anchor of RFC 0007", () => {
  const removed =
    /0007-relay\.md#(r\d+-|decisions|rejected-options|conflicts-with-genesismd-and-rfc-0001|open-questions)/;
  const found = documents().filter((path) => removed.test(read(path)));
  assert.deepEqual(found, []);
});

test("the status, the discussion and History", () => {
  assert.equal(
    line(read(RFC), "**Status.**"),
    "**Status.** Decided by the owner (Carsten, 2026-11-10).",
  );
  const feedback = line(read(RFC), "**How to give feedback.**");
  assert.ok(links(feedback, DISCUSSION), "the discussion #147 is not linked");
  assert.ok(links(feedback, "README.md#giving-feedback"));
  const lines = part("history")
    .split("\n")
    .filter((l) => l.trim());
  assert.deepEqual(lines, [`- ${PUBLISHED}`]);
  assert.match(read(RFC), /crates\/ncdb-relay/);
});

// Each former decision stands in its section. The comment names the decision
// as the old text numbered it, so the PR's mapping table can be checked
// against this file.

test("the transport, its limits, and where TLS ends", () => {
  // R1, R4; rejected: HTTP and server-sent events, gRPC, WebTransport; NOC-159
  holds(
    part("transport-and-encoding"),
    [
      /server-sent events/,
      /gRPC/,
      /WebTransport/,
      /[Ll]imits per backend/,
      /loopback/,
      /Caddy/,
      /nginx/,
      /API Gateway/,
    ],
    "Transport and encoding",
  );
});

test("the keys of each message, and the forms not taken", () => {
  // R12
  holds(
    part("the-keys-of-each-message"),
    [/[Tt]ext keys for every field/, /[Nn]umbers picked by hand/],
    "The keys of each message",
  );
});

test("a connection authenticates with the member key, not a bearer token", () => {
  // R2; rejected: bearer tokens
  holds(
    part("opening-a-connection"),
    [/[Bb]earer tokens/, /no token can be stolen and replayed/],
    "Opening a connection",
  );
});

test("a member added while connected learns it at once", () => {
  // R13
  holds(
    part("subscribe-follow-a-space-live"),
    [/on a timer/, /next reconnect, within 2 hours/],
    "Subscribe",
  );
});

test("the person-key store: signed records, and a writer who has since left", () => {
  // rejected: unsigned records; NOC-133 point 2
  holds(
    part("keys-and-the-person-key-store"),
    [
      /[Uu]nsigned person-key records/,
      /has since left/,
      /uploader must still be a current member/,
      /re-wrap written before a removal stays valid/,
      /does not strand a key/,
    ],
    "Keys and the person-key store",
  );
});

test("the largest change", () => {
  // R5
  holds(
    part("large-changes"),
    [/16 MiB/, /4 MiB/, /multipart/],
    "Large changes",
  );
});

test("what the relay checks: who creates a space, revoked members, equivocations", () => {
  // R6, R7; rejected: judging by arrival, refusing equivocations, folding into its own format
  holds(
    part("what-the-relay-checks"),
    [
      /allowlist/,
      /[Aa]nyone who can reach the relay/,
      /invitation code/,
      /by the relay's order/,
      /under key generations before the removal/,
      /choose a winner/,
      /rebuildable from the log/,
    ],
    "What the relay checks",
  );
});

test("what the relay sees: connection records and log retention", () => {
  // R9, R11
  holds(
    part("what-the-relay-sees"),
    [/always kept after the connection ends/, /90 days/, /1 day/],
    "What the relay sees",
  );
});

test("the backend interface: one core, dense positions", () => {
  // rejected: checks inside each backend, positions from timestamps or Streams
  holds(
    part("the-backend-interface"),
    [
      /[Cc]hecks inside each backend/,
      /[Tt]imestamps tie and run backwards/,
      /DynamoDB Streams' order/,
    ],
    "The backend interface",
  );
});

test("the AWS backend: compute, region, tables, frames and backups", () => {
  // R3, R8, R10; rejected: one table, all frames in S3 or DynamoDB, AppSync, EC2, the same epoch
  holds(
    part("the-aws-backend"),
    [
      /us-east-1, N\. Virginia/,
      /[Oo]ne DynamoDB table for everything/,
      /[Aa]ll frames in S3/,
      /[Aa]ll frames in DynamoDB/,
      /AppSync/,
      /EC2/,
      /DynamoDB Streams and a second Lambda/,
      /[Nn]o backups/,
      /same epoch/,
    ],
    "The AWS backend",
  );
  holds(part("what-it-costs"), [/ECS Fargate/], "What it costs");
});

test("where this RFC makes GENESIS.md, RFC 0001 and RFC 0002 more precise", () => {
  // the former conflicts 1 to 4
  const agreement = part("agreement-with-genesismd-and-rfc-0001");
  for (const target of [
    "0002-change-and-log-format.md#what-the-relay-can-see-and-what-it-cannot",
    "#what-it-costs",
    "0002-change-and-log-format.md#on-the-relay",
  ]) {
    assert.ok(
      links(agreement, target),
      `the agreement does not link ${target}`,
    );
  }
  holds(agreement, [/[Uu]se case 4/, /[Uu]se case 5/], "Agreement");
  assert.doesNotMatch(agreement, /Resolved \(NOC-47\)/);
});

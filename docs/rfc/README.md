# RFCs

NoCentralDB is designed in the open. Each RFC here describes one part of the design. It says at its top whether it is a draft or decided, and it states the current design as one complete text. Its "History" section, at the end, lists by date what changed and in which pull request. RFCs from before this rule are being rewritten this way; until then, they carry their later changes as numbered sections after the main text.

| RFC | Title |
| --- | --- |
| [0001](0001-nocentraldb.md) | NoCentralDB: the whole design on a few pages. Start here. |
| [0002](0002-change-and-log-format.md) | The change and the log format |
| [0003](0003-keys-and-cryptography.md) | Keys and cryptography |
| [0004](0004-merge-rules-basic-types.md) | Merge rules for basic values, sets and links |
| [0005](0005-schema-and-model-lock.md) | The schema language and `model.lock.json` |
| [0006](0006-local-view.md) | The local view on SQLite, natively and in the browser |
| [0007](0007-relay.md) | The relay protocol and its AWS backend |
| [0008](0008-membership.md) | Membership, roles and the first change of a space |
| [0009](0009-sdk-api.md) | The SDK API for apps |

## Giving feedback

Feedback on any RFC goes to GitHub Discussions, category [RFC feedback](https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback).

- **Reply in the RFC's own discussion** when it has one. It is named after the RFC, such as [RFC 0001: NoCentralDB](https://github.com/nocentraldb/nocentraldb/discussions/21). Keep to one topic per reply, so that each point gets its own thread of answers.
- **Start a new discussion** in the category when your point needs room of its own, or when the RFC has no discussion yet. The form asks which RFC and section you mean, your situation, and where the design would break for you.
- **Open a pull request** against the RFC's file to change its text: a typo, an unclear sentence, a wrong link.

Tell us about your app, your situation and where the design would break for you: reports of problems help us more than feature requests. The [FAQ](../faq.md) may already answer your question. Security problems never go into a discussion: see [SECURITY.md](../../.github/SECURITY.md).

A decided RFC still takes feedback. Its decisions change only through a decided change to its text, so a point against one takes longer, and the answer may be that we keep the decision and say why.

## How a decided RFC changes

A decided RFC changes by rewriting its text, never by adding to it. A pull request that changes a decided RFC does three things:

1. The change is written into the text where it belongs, so that the RFC reads as if it had been written that way from the start. The pull request adds no amendment section and no mark such as "(Amendment 3)": an RFC has no amendment sections.
2. The decision goes under "Decisions", in short form: what was chosen, who decided and when, why, and each option not taken in one line.
3. A dated line in "History" names the pull request, and the discussion the change came from, if any.

A change to a decision waits for the owner, as every RFC decision does. A withdrawn decision stays under "Decisions", marked as withdrawn. Git keeps every earlier version of the text, and the History lines lead to them.

## What happens to your feedback

We read every reply and answer within a week. Each point ends in one of four ways, and the answer in the discussion says which:

1. **A change to the RFC's text.** The change is written into the RFC as [How a decided RFC changes](#how-a-decided-rfc-changes) says, and its History line links back to the discussion. A change to a decision waits for the owner, as every RFC decision does.
2. **A ticket.** Work the point asks for, outside the RFC's text, becomes a ticket in the project's tracker. The tracker is not public, so the reply says when the work ships.
3. **An answer in the FAQ.** A question the RFC already answers, but not clearly enough, becomes a FAQ entry.
4. **No change, with the reason.** The design keeps what it says, and the reply explains why.

When an RFC's next draft or a change to its text is merged, its discussion gets a short summary of what changed and which replies led to it. Watch the repository's releases, or the discussion itself, to hear about it.

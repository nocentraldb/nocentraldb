# RFCs

NoCentralDB is designed in the open. Each RFC here describes one part of the design, and each one says at its top whether it is a draft or decided, and what changed since it was first written.

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

## Giving feedback

Feedback on any RFC goes to GitHub Discussions, category [RFC feedback](https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback).

- **Reply in the RFC's own discussion** when it has one. It is named after the RFC, such as [RFC 0001: NoCentralDB](https://github.com/nocentraldb/nocentraldb/discussions/21). Keep to one topic per reply, so that each point gets its own thread of answers.
- **Start a new discussion** in the category when your point needs room of its own, or when the RFC has no discussion yet. The form asks which RFC and section you mean, your situation, and where the design would break for you.
- **Open a pull request** against the RFC's file to change its text: a typo, an unclear sentence, a wrong link.

Tell us about your app, your situation and where the design would break for you: reports of problems help us more than feature requests. The [FAQ](../faq.md) may already answer your question. Security problems never go into a discussion: see [SECURITY.md](../../.github/SECURITY.md).

A decided RFC still takes feedback. Its decisions change only through an amendment, so a point against one takes longer, and the answer may be that we keep the decision and say why.

## What happens to your feedback

We read every reply and answer within a week. Each point ends in one of four ways, and the answer in the discussion says which:

1. **An amendment to the RFC.** The RFC gets a new amendment section and a changelog line that links back to the discussion. A change to a decision waits for the owner, as every RFC decision does.
2. **A ticket.** Work the point asks for, outside the RFC's text, becomes a ticket in the project's tracker. The tracker is not public, so the reply says when the work ships.
3. **An answer in the FAQ.** A question the RFC already answers, but not clearly enough, becomes a FAQ entry.
4. **No change, with the reason.** The design keeps what it says, and the reply explains why.

When an RFC's next draft or amendment is merged, its discussion gets a short summary of what changed and which replies led to it. Watch the repository's releases, or the discussion itself, to hear about it.

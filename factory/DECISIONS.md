# DECISIONS.md — where the decision log lives

The decision log is the folder [`factory/decisions/`](decisions/), one file per decision. Nobody writes to this file any more; until NOC-27 it held every entry, and git history keeps that version.

## Reading it

```sh
pnpm decisions                      # every decision in full, in date order
pnpm decisions --index              # one line each: date, ticket, title
pnpm decisions --ticket NOC-22      # only this ticket's decisions
pnpm decisions --topic crypto       # only this topic's decisions
pnpm decisions --since 2026-11-01   # only decisions made on or after this date
```

The filters combine with each other and with `--index`. `ls factory/decisions/` lists them by date too.

## Writing one

Add a new file, `factory/decisions/YYYY-MM-DD-<ticket>-<slug>.md`, for example `2026-11-08-NOC-22-rfc-0002-amendment-1.md`, or `2026-11-07-no-ticket-tickets-live-in-linear.md` when there is no ticket. When the heading names several tickets, the file name takes the first. The slug is the title in lowercase words joined by `-`.

```markdown
# 2026-11-08 · NOC-22 · RFC 0002 Amendment 1: every recommendation accepted

- Decided by: Carsten
- Why: one line
- Instead of: the option not taken, one line
- Topics: rfc-0002, log
- Supersedes: 2026-11-07-NOC-6-some-older-decision.md
```

- **Topics** is required: one or more lowercase tags, separated by commas, so a reader can find every decision on a subject.
- **Supersedes** is optional: the file names of earlier decisions this one replaces, separated by commas. The PR that adds it deletes those files; git history keeps them.

Only decisions that still bind work after their ticket is done belong here; the title states the decision itself, and a choice that matters only for one ticket goes on that ticket (see "The decision log" in FACTORY.md).

`pnpm gates` checks every file (the task `repo:decisions`): the name pattern, the heading's date and first ticket against the file name, the four required fields, a title that names a decision rather than a review's outcome or an answer to questions, and that each `Supersedes:` names a decision file that is no longer in the folder.

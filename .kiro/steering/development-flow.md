---
inclusion: always
---

# How work is developed in this project

Work moves through three flows, in this order: **planning → coding → merging**. A review round
is a step inside planning (judging a design) and inside coding (judging a change). The roles are
Kiro skills (`pm`, `coder`, `verifier`, `code-quality`, `test-quality`, `integrity`, `finisher`,
`merger`); each skill describes its own role. This file is the ONE place that describes the
order of the steps, so no skill carries a second copy of it.

Kiro has no channel engine. The session that runs a flow is the **orchestrator**: it starts
every step as its own sub-agent that loads the role's skill, hands it exactly the inputs listed
below and nothing else, reads the result and routes it. The orchestrator never does a step's
work itself, and never shows a checker what the producer said about its own work. Board items
are `nxf` tickets; everything durable is written in English.

Small work, a product opinion or a question goes to the `pm` skill directly as a conversation;
then no review runs. The flows below are for work that must be checked.

## Planning — an idea becomes a judged plan on the board

| # | Step | Role | Handed | Ends with | Goes back |
|---|---|---|---|---|---|
| 1 | design | `pm` (in the main session) | the idea, or the requester's answer to the PM's last question | the epic as the SPECIFICATION, appended as a note "Specification as handed to be judged, round <n>"; no tickets yet | — |
| 2 | judge | review round at a DESIGN | the epic, its board item, the code it touches; from round 2 the previous consolidated review | a consolidated review; `needs_rework` only for a High or Critical | to step 1, at most **2** judged rounds |
| 3 | cut | `pm`, same session | the consolidated review (it must arrive; the PM never reconstructs it) | the epic, tickets in order, every Medium of every round in a ticket's definition of done marked `must-fix (design review)` | — |

- Questions, a choice of approaches or the approval of a design are asked of the user, never
  handed to the review as if they were a design. A spike is answered, not designed.
- After the second judged round the design does not go round again: it goes to the user with
  the open points. The user accepts it over the verdict (with their decisions) and it is cut, or
  names what to rework and the design starts over.
- Planning ends with the plan on the board. Starting the work is a new request (the PM's
  "drive" mode).

## Coding — one ticket, from nothing to a pull request

Address it with one ticket id. One ticket is in flight at a time: the working copy is shared,
and only one step writes to it at any moment.

Before step 1, record the run start (`date -u +%Y-%m-%dT%H:%M:%SZ`); the verifier and the
reviewers need it to judge flake registrations.

| # | Step | Role | Handed | Ends with | Goes back |
|---|---|---|---|---|---|
| 1 | build | `coder` | the ticket id; on a ride-along the open pull request; on rework the reason it came back (quoted, or as a note on the ticket) | a clean, committed branch, a session report as a note on the ticket, a short reply naming branch and commit | — |
| 2 | verify | `verifier` | ONLY the ticket id, the branch and the run start — never the coder's reply | `green`, `green except for registered flakes` (that exact first line) or red, with the gates log path in `.nxf/review/` and the commit | red → step 1, at most **3** passes |
| 3 | judge | review round at a CHANGE | branch and base, ticket id, run start, the verifier's reply | a consolidated review; `needs_rework` for any Medium or worse | → step 1, at most **3** passes |
| 4 | finish | `finisher` | the ticket id, the verifier's reply AND the consolidated review (both required) | a pull request (or the reason there is none), base and verified commit | tree moved or not finishable → step 1, at most **3** passes |

- A pass that goes back starts a fresh `coder` sub-agent with the ticket and the reason it came
  back; the session report on the ticket carries the continuity. Every pass that goes back runs
  verify, judge and finish again from the new tree.
- A step that reaches its cap, or any role that stops and asks, stops the flow: take it to the
  user instead of looping.
- A public web page needs the `art-director`'s direction before it is built and its verdict on
  the handed-over commit; the `coder` arranges both, the `finisher` checks the verdict.
- The ticket is not closed here. It closes when its work is merged.

## Merging — does it go in now, or does the next ticket ride along?

After every coding run, before the next ticket starts, delegate to a sub-agent that loads
`merger`, handed the finished ticket, the next planned ticket (or "last ticket of the epic"), the
branch and pull request, and the consolidated review verbatim (the merger cannot read the review
otherwise). It answers `MERGED` (merged, delivered result proven green, tickets closed) or `HOLD`
(the next ticket rides along in the named pull request, and the coder builds on that branch).

## The review round

Three reviewers — `code-quality`, `test-quality`, `integrity` — each in its own sub-agent, in
parallel, read-only. None sees another's review, the author's account or a self-review. Each
skill carries its own rules for a CHANGE and for a DESIGN; the orchestrator says which mode and
hands the inputs from the tables above.

Then a fourth sub-agent, handed ONLY the three reviews, consolidates them. The author never
summarises criticism of their own work. Before you start that sub-agent, read
`.kiro/steering/review-consolidation.md` and hand it those rules. (For a design, the `pm` skill's
`references/design-review.md` holds the same rules for the design mode.)

The work goes back if ANY reviewer's own verdict line says `needs_rework`. The consolidated
verdict reports the count; where it disagrees with the reviewers' own lines, say so.

Changes to the skills, steering or project rules themselves are judged by a sub-agent that loads
`declaration-check`, handed the change and its board item.

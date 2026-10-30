# Rung 6 — drive the epic to done, ticket by ticket

Read this completely before you hand over the first ticket of an epic, and follow it for every
ticket after that.

If the requester says go, you now RUN the epic. You do not ask between tickets: hand each
ticket to the coding flow, with the ticket's id as the subject, saying what to build and where
the plan is.

## Before EVERY hand-over: check the ticket's hygiene

This is not optional and it is not a formality.

A ticket is a claim about the world made on the day it was written. The world moved. Measured in
this workspace: a ticket sat for eighteen days, and later work had made three of its acceptance
criteria pointless — the briefing that went out named two of them, missed a third, and wrongly
carried a fourth as obsolete. Nobody noticed until afterwards.

So, for the ticket you are about to hand over:

- **Is it still true?** Read its acceptance criteria against TODAY'S SOURCE, not against your
  memory of the board. Every criterion that no longer applies, you say so — on the ticket, with
  the reason — before you hand anything over. A criterion you cannot verify either way is a
  question for the requester, not a guess.
- **Is it actually free?** Look at what it depends on and what it blocks. A dependency that
  closed since may have satisfied part of it; one still open means this ticket is not the next
  one, whatever the ready list says. One exception: a ticket stays open until its work is
  merged, so after a HOLD the finished ticket is still open — and the next ticket the `merger`
  named to ride along is free all the same, although the ready list will show it as blocked.
- **Does it still describe ONE thing?** Tickets grow notes. If the notes have quietly turned it
  into two pieces of work, cut it now — a coder handed two jobs does the first one and reports
  both.
- **Is what it asks for still what the epic wants?** The epic is the intent; the ticket is one
  step towards it. If they have drifted apart, the epic wins and the ticket gets corrected.

Where you re-word an old criterion, DERIVE IT FROM THE CODE AS IT STANDS. Reconstructing it from
the ticket's own history is how the wrong version travels one step further.

## One ticket through the coding flow

One ticket at a time, in `nxf next` order. The working copy is one: a second ticket built at the
same time would build on top of the first and buy nothing — sequential is not caution here, it
is the only thing the shared working copy permits.

Hand each ticket to the coding flow ONCE, as the project's steering describes it — the `coder`
builds it, the `verifier` runs the gates, the three reviewers (`code-quality`, `test-quality`,
`integrity`) judge the change, the `finisher` puts it up — and always in sub-agents, never in
your own context: you have no business in the working tree. One result comes back per ticket.
The flow checks its own work on the way; what that means in detail is in the steering — do not
carry a second copy of it in your head, because the copy is what goes stale. A run that comes
back with no pull request is not half-done work for you to chase: read the result, it says why.

**What the run could not settle comes back to YOU, and settling it is a NEW run.** When the
result carries Critical or High findings the branch does not already address, the ticket is not
done: hand the same ticket over again.

Carry the findings INTO that hand-over, in full. The next run starts a fresh coder that has no
memory of the last one and cannot see its conversation; a hand-over that only says "fix the
review findings" gives it nothing to fix. Quote them, or put them on the ticket as a note first
and point at it — the note is the better half, because the board is synced and a conversation
is a log.

Each run leaves a session report on the ticket (a note); the result itself is a short pointer.
For each run, in this order:

1. **Verify what it claims.** `nxf show <ticket>`, `git log`, and the one command that proves
   the load-bearing claim. A report is a claim until you have checked it. Say in your own final
   report which claims you checked and which you took on trust.
2. **Carry the lesson forward.** If the ticket was wrong, ask whether the SAME wrongness sits in
   a later ticket, and fix those now rather than discovering it eleven times. Put the lesson as
   a note on the epic, not only on the ticket that suffered it.
3. **Handle what came back open.** Something labelled "for the human" that can wait is a TICKET,
   not an interruption: `nxf create --type bug|chore --title "…" --priority P…`, and it goes into
   your final report under open points. Something labelled "for the human" that genuinely blocks
   the next ticket is the ONE case where you stop and ask.
4. **Ask the `merger` what happens to the work.** You do not decide this and neither does the
   coder. Delegate it to a sub-agent that loads the `merger` skill, with the finished ticket as
   the subject, and hand it:

       Finished: <finished ticket id> — <one line on what it does>
       Planned next: <next ticket id> — <one line, and which files it touches>
       Branch: <branch>   Pull request: <number and URL from the finisher, or why there is none>
       The session report is a note on <finished ticket id>.
       Review verdict: <the grades, the number of findings per severity, and what was fixed vs.
       declined with reasons — VERBATIM from the consolidated review, not summarised>.

       Question: does this go in now, or should the next ticket ride along in the same pull
       request?

   **Carry the review verdict over yourself.** The `merger` was not part of the review and
   cannot read it — it sees only what you hand it and what is on the board. A reference to a
   review it cannot open blocks it; that happened twice on the test bench. Either quote the
   consolidated review in the hand-over, or attach it to the ticket as a note first and name the
   note id.

   Name the next ticket even when you think the answer is obvious — the whole question is
   whether these two belong together, and the `merger` cannot judge that without knowing what
   comes next. If this was the LAST ticket of the epic, say so instead of naming a next one.

   Two verdicts come back, and they lead to different next steps:
   - **MERGED** — the next ticket starts from a clean base. Hand it over normally. If the
     `merger` left a ticket open because the run after the merge was red, that is a stop:
     relay it and ask. Once the default branch is green again, closing that ticket is yours,
     with the merge commit the `merger` named in the reason.
   - **HOLD** — the next ticket rides along in the named pull request. Say that in the
     hand-over, with the pull request number, so the coder pushes onto that branch instead of
     opening a second one.

   If the `merger` stops and asks for a decision instead of a verdict, that stops you: relay it
   to the requester with your own reading of it and ask.
5. **Hand over the next ticket** — ONE, and only after step 4 has answered.

## Exactly one ticket in flight

Never start a second coding run while one is still open, and never start one before the
`merger` has answered for the previous ticket. The order is not a preference: the `merger`'s
whole question is whether the finished ticket and the NEXT one belong in the same pull request,
and a next one that is already being built has answered that question by force.

So before every hand-over to the coding flow, check that nothing of yours is in flight — no
coding run still open and no finished ticket still waiting for the `merger`'s answer.

If two tickets are genuinely independent and you are tempted to run both — do not. Pick the one
that de-risks more, and say in your closing report why the other waited.

## What stops you, and only this

A coder that stops and asks for a decision; a decision that blocks the next ticket and is not
yours; or no unblocked ticket left. Everything else you decide and record. A question you could
have filed as a ticket costs the requester a round trip and buys nothing — that is the failure
mode this rung exists to prevent.

End of each round, while more remain: `[pm-state: plan-on-board]`. When no unblocked ticket is
left, go to rung 7 (`references/closing-report.md`).

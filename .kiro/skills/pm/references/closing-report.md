# Rung 7 — the closing report

Read this when no unblocked ticket of the epic is left, before you write the report.

Write ONE report over the whole epic. Same split as the coder's: **the report goes on the EPIC
as a note** (`nxf note add <epic id> -`), and your reply carries the headline plus the pointer,
not the report again.

Write it for a person who is not an engineer and did not watch any of it happen. No code, no
function names. Name things the way the product does, and cite ids so they can look them up.

    ## TL;DR
    One or two sentences. What the epic now delivers.

    ## What landed
    - <ticket id> — <in plain words what it does, and why that matters to a user>
    - Only what ACTUALLY landed. Done-but-unmerged is said so, not dressed up.

    ## What did not go as planned
    - <every surprise, wrong assumption, blocker and trade-off across the whole epic>
    - Include the ones that were YOURS. A plan that was wrong and got corrected is the most
      useful thing in this section, and hiding it costs the next epic.

    ## Where the work drifted from what was asked
    The requester described a product before any of it existed. Say plainly where what got
    built differs from that description — in scope, in behaviour, in emphasis — and whether the
    drift was decided or just happened. This section is the reason the report exists: it is
    cheaper to notice a divergence here than three epics later. If there was none, say so in
    one line, and mean it. The written standard is `README.md`; read it before you write this
    section, not from memory of what the requester once said.

    ## Still open
    - <each open point, labelled tracked (<nxf id>) or not tracked, with your recommendation>
    - Anything the human must decide belongs here, with how urgent it actually is.

    ## How to check it yourself
    - The command to run it, and what should be seen — not "read the diff".
    - For each closed ticket, the ONE thing that would convince a sceptic it is done.

Then reply, short: a TL;DR, a pointer at the board — `nxf show <epic id>`, note `<note id>` —
and, under "Open and with you", the one or two things that need a human.

End: `[pm-state: done]`

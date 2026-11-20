# 2026-11-20 · NOC-383 · The relay hands a removed member the control change that removes it, and its late changes become rejected on its own copy

- Decided by: Carsten, 2026-11-20, on NOC-383 ("Option A")
- Why: RFC 0008 ("A member removed while offline who keeps writing", "On the laptop itself") promised that the removed member folds its own removal, so that its changes above the cutoff become rejected on its own copy too and the SDK tells the app which of its writes no member accepts. RFC 0007 served a removed member nothing, so its copy kept those changes folded and differed from every other copy (found by NOC-381's test, #391). The relay now reveals to the removed member that one control change, and nothing else of the space; RFC 0007's "Subscribe" and "The errors" say so, and both relays, the local one and the one on AWS, do it.
- Instead of: amending RFC 0008 so that a removed member's copy keeps its late changes as local information only, and the SDK says only that the member was removed (option B), which leaves the app unable to show the person what was lost.
- Topics: relay, membership, rfc-0007, rfc-0008

# 2026-11-08 · NOC-23 · A revocation wins over a concurrent grant, and seniority decides a cycle of concurrent revocations

- Decided by: Carsten (RFC 0008 decisions M2 and M4)
- Why: a lost grant is cheap to repeat, while a lost removal leaves a stolen device writing; concurrent role changes leave the member with the lower role. In a cycle of concurrent revocations the senior admin's revocation wins, seniority being the depth of the admin grant with the creator first, so a compromised admin cannot cancel its own removal by removing the remover. Verdicts can therefore change while concurrent control changes arrive, the SDK reports each change, and a late revocation un-folds what a stolen device wrote meanwhile.
- Instead of: the grant winning, as an addition does in RFC 0004's sets; both revocations of a cycle taking effect (a compromised admin takes an honest one down with it); neither taking effect (it cancels its own removal); no late control change ever un-folding anything.
- Topics: rfc-0008, membership, convergence

The order, seniority and edges are in `docs/rfc/0008-membership.md`, "The function", with the cases in "Concurrent add and remove of one member" and "Two admins who remove each other".

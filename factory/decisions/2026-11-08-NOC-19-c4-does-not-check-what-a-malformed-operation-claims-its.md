# 2026-11-08 · NOC-19 · C4 does not check what a malformed operation claims its author had seen

- Decided by: coding agent (NOC-19)
- Why: a malformed operation's body can claim to have seen anything, and the harness cannot tell a lie from a claim; C4 still requires every copy to give it the same outcome, and C1/C5 still require convergence.
- Instead of: holding malformed replacers to the simulated member's view, which would reject every type that, like RFC 0004's rules, trusts the dependencies a change carries.
- Topics: convergence, testing

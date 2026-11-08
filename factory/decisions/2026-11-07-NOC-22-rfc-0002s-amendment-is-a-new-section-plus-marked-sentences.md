# 2026-11-07 · NOC-22 · RFC 0002's amendment is a new section plus marked sentences, and keeps "refused by the relay" apart from "rejected"

- Decided by: coding agent (NOC-22)
- Why: the amendment needs one place for the owner's decisions D10 to D12, and the old sentences must not contradict it, so each changed sentence carries an *(Amendment 1)* mark and a dated changelog line sits at the top. "Rejected" is a log state that only the member's own checks set; the relay's refusal is recorded beside it, so the state stays a function of the changes a member holds (principle 2).
- Instead of: rewriting the sections silently, or letting RFC 0007's "marked as rejected" for a relay refusal set the log state.
- Topics: rfc-0002, relay, log

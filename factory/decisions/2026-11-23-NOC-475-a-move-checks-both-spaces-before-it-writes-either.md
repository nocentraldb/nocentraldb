# 2026-11-23 · NOC-475 · `move_objects` checks both spaces and every object before it writes either change

- Decided by: coding agent, NOC-475
- Why: RFC 0016 says a refused move writes nothing. The member's right to write in both spaces, the relay still serving both, and each object's checks run before the pending record and the new space's change, so the only refusal left between the two changes is one the relay gives later, which `space.status` reports as for any write.
- Instead of: letting `write_change` check each space as it writes, which could write the new space and then refuse the old.
- Topics: rfc-0016, move, sdk-rust, errors

# 2026-11-08 · NOC-22 · A change passes its checks as a whole, a failed check keeps it rejected, and a relay refusal leaves it folded

- Decided by: Carsten (RFC 0002 decisions D10 to D12)
- Why: D10: signature, membership and decryption cover the change as a whole, and only an operation on a collection or field the client's model does not know is parked on its own, so old clients do not fall further behind with every schema addition. D11: a change that fails a check is kept as rejected, with a reason, and checked again when a control change of its space arrives, because a member cannot tell "not yet" from "never" and dropping it would make members differ by order of arrival. D12: a change the relay refuses after the member folded it stays folded and the member records the refusal beside it, because the state must not depend on the relay's answer, which no other member sees.
- Instead of: D10, one unknown operation parking the whole change; D11, rejected only for checks that can never pass with "waiting" parks for the rest, or dropping a failed change; D12, making the refused change rejected, or pushing it until the relay accepts it.
- Topics: rfc-0002, log, change-format, convergence, relay

`docs/rfc/0002-change-and-log-format.md`, "What a member does with a change it receives", states the steps, the states and the re-checks. RFC 0008 later added that a member pushes a refused change once more when its own fold newly accepts it after rejecting it (see the NOC-120 entry).

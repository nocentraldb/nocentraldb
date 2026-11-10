# 2026-11-10 · NOC-136 · A write checks the role the member's copy folded, after the space is held

- Decided by: coding agent (NOC-136)
- Why: RFC 0009's error table refuses a write from a reader with `notAllowed` and one from a member that is no longer one with `notMember`, and RFC 0008 gives the right to write content to writers and admins. `space.write` looks up the member's role in the membership its copy folded, through the same lookup as `members().add` (`members::holds`), and refuses before it seals, so nothing reaches the log. It checks after `NotFound`, `SpaceOfAnotherModel` and the relay's `not_member` (NOC-158), so those answers stay as they were.
- Instead of: letting the write go on and reporting the core's rejection of the member's own change, which appended the change to the log first and then panicked as a bug of the SDK.
- What this does not do: the check reads only what the copy folded. A member whose copy has not folded its removal, and whose relay has not answered `not_member`, still writes; the relay and every other member reject that change. A removed member whose copy folded a rotation it holds no key for is answered `NotFound`, as `spaces().get` answers it, not `NotMember`, since the key check runs first; rotation on removal is not built in the SDK yet.
- Topics: sdk-rust, rfc-0008, rfc-0009, errors

# 2026-11-12 · NOC-270 · An upload is marked filled before its frame is written to S3, and a take reads the frame before it deletes the item

- Decided by: proposed by the coding agent (NOC-270); the owner decides with the tick on PR #275
- Why: two fills of one upload race on one S3 key; marking the item filled on the condition that it is not lets exactly one write the frame, as "filled once" needs. A take that finds the item filled but no frame yet answers none and deletes nothing, and a take deletes the item on the condition that it is still filled and its own, so a frame is taken once. A fill whose S3 write fails takes its mark back.
- Instead of: writing the frame first, where the loser of a race could overwrite the winner's frame; or a key per fill, which RFC 0007's `uploads/<upload id>` does not have.
- Topics: relay, aws, uploads

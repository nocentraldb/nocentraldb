# 2026-11-21 · NOC-427 · The relay remembers each space's resolve in its process, and stores nothing new

- Decided by: coding agent (NOC-427)
- Why: each push of a control change and each put of person-key records read and resolved every control change of the space from scratch, so a new person cost the relay the square of the persons (about 3 s at 800 on one core). A memory per process, the control changes read so far and a `membership::Resolver`, makes a warm instance read and judge only what is new (about 18 ms at 800), and answers stay a function of the log: a new relay epoch, or a log that no longer holds the last entry read at its position, makes it read the space again. It holds at most 16 spaces and at most the sum of squared control changes of one space of 1,200, least recently used forgotten first, since a space's memory grows with the square of its control changes and a Lambda has 256 MB.
- Instead of: a persisted snapshot of the resolved state in a new DynamoDB item (a new stored format to version and keep equal to the log, which the owner would have to decide), or no memory and a larger Lambda
- Topics: relay, membership, performance, aws

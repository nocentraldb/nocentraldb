# 2026-11-23 · NOC-417 · `RelayStack` holds the `Relay` construct under the id `Default`, so no logical id changes

- Decided by: coding agent (NOC-417)
- Why: CDK leaves a path component named `Default` out of logical ids, so every resource of the deployed stack keeps its logical id and its properties, and a deploy replaces or renames nothing; only the `aws:cdk:path` metadata gains `Default/`. The committed template snapshot stays byte-identical, which is the test. Later changes to the construct must keep `RelayStack`'s logical ids, since the tables and the bucket have fixed names and are retained.
- Instead of: a construct id such as `Relay`, which would rename every logical id and make CloudFormation try to replace named tables; or overriding each logical id by hand.
- Topics: relay, aws, cdk, infrastructure

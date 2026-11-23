# 2026-11-23 · NOC-417 · The relay's settings are one `RelaySettings` object, and `relayEnvironment` turns them into its `NCDB_*` variables

- Decided by: coding agent (NOC-417)
- Why: a new setting of the relay, such as admission's `NCDB_ADMISSION_KEYS` (NOC-469), goes into one place, `infra/aws/src/relay-settings.ts`: a field of `RelaySettings`, its check, and its variable. The construct takes them as `settings`, apart from where it is deployed (`stage`, `lambdaPackage`), and refuses malformed values when it synthesizes.
- Instead of: flat props on the construct next to its deployment values, or variables set inline in the construct.
- Topics: relay, aws, cdk

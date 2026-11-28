---
type: feature
tickets: [NOC-532]
components: [relay]
---
## Problem
An operator of the `Relay` construct could not keep connection records after a connection ended, for abuse investigation, nor keep the relay's logs longer than 14 days, although RFC 0007 lets it do both.

## Use
Set the days in the construct's settings; both are optional, and left out the relay keeps no connection records after a connection ends and keeps its logs 14 days:

```ts
new Relay(stack, "Relay", {
  stage: "myapp",
  settings: {
    relayId: "…",
    allowedAccounts: ["…"],
    keepConnectionRecordsDays: 30,
    logRetentionDays: 90,
  },
  lambdaPackage: "…",
});
```

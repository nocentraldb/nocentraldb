---
type: feature
tickets: [NOC-469]
components: [relay]
---
## Problem
An app that runs the relay in its own AWS stack could not set the admission keys the relay takes admission tokens from.

## Use
Pass the public half of your app's admission key, 64 lowercase hex digits, in the construct's settings; the function gets it as `NCDB_ADMISSION_KEYS`:

```ts
new Relay(stack, "Relay", {
  stage: "myapp",
  settings: { relayId, allowedAccounts: [], admissionKeys: ["…"] },
  lambdaPackage,
});
```
